# FANatic Settlement — Technical Documentation

## Trustless World Cup Prediction Market Architecture

**Anchor Program:** `fanatic_settlement`
**Program ID:** `2Ju8T8v7QkSXxTpfxTQhfJNKW1DwGU8Eq4dBzz8DANoG`
**DevNet Explorer:** https://explorer.solana.com/address/2Ju8T8v7QkSXxTpfxTQhfJNKW1DwGU8Eq4dBzz8DANoG?cluster=devnet
**Anchor Version:** 0.31.1
**Solana Version:** 2.1.x (Agave 3.x compatible)
**Date:** 2026-06-24

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [PDA Seed Design](#2-pda-seed-design)
3. [Account Structures](#3-account-structures)
4. [Instruction Specifications](#4-instruction-specifications)
5. [TxLINE CPI Flow for validate_stat](#5-txline-cpi-flow-for-validate_stat)
6. [TxLINE Endpoints Used](#6-txline-endpoints-used)
7. [Payout Mathematics](#7-payout-mathematics)
8. [Security Model](#8-security-model)
9. [Error Codes](#9-error-codes)
10. [Deployment Guide](#10-deployment-guide)

---

## 1. Architecture Overview

```
                        OFF-CHAIN                        ON-CHAIN (Solana DevNet)

 World Cup  ──────► TxLINE SSE Stream            ┌─────────────────────────────────┐
 Data Feed          (Real-time match stats)      │   FANatic Settlement Program    │
                    │                             │                                 │
                    │  Anyone can                 │  ┌─────────────────────────┐    │
                    │  submit Merkle              │  │ PlatformState PDA       │    │
                    │  proof on-chain ───────────►│  │ seeds=["platform"]      │    │
                    │                             │  └───────────┬─────────────┘    │
                    │                             │              │                  │
                    │                             │  ┌───────────▼─────────────┐    │
                    │                             │  │ MatchRegistry PDA       │    │
                    │                             │  │ seeds=["match", txline_  │    │
                    │                             │  │   match_id]             │    │
                    │                             │  └───────────┬─────────────┘    │
                    │                             │              │                  │
                    │                             │  ┌───────────▼─────────────┐    │
                    │                             │  │ MarketAccount PDA       │    │
                    │                             │  │ seeds=["market",        │    │
                    │  CPI call to                │  │   match, event_id]      │    │
                    │  TxLINE ◄───────────────────│  │ - 8-outcome pools       │    │
                    │  validate_stat              │  │ - stat_key bridge       │    │
                    │                             │  └───────────┬─────────────┘    │
                    │                             │              │                  │
  Fan User ────────► place_prediction ───────────►│  ┌───────────▼─────────────┐    │
                    (SOL transfer)                 │  │ PredictionPosition PDA  │    │
                    │                             │  │ seeds=["prediction",    │    │
                    │                             │  │   market, user]         │    │
                    │                             │  └─────────────────────────┘    │
                    │                             │                                 │
                    │                             │  ┌─────────────────────────┐    │
                    │                             │  │ OracleProof PDA         │    │
                    │                             │  │ seeds=["oracle_proof",  │    │
                    │                             │  │   market, txline_id]    │    │
                    │                             │  └─────────────────────────┘    │
                    │                             │                                 │
                    │                             │  ┌─────────────────────────┐    │
                    │                             │  │ TreasuryVault PDA       │    │
                    │                             │  │ seeds=["treasury",      │    │
                    │                             │  │   "platform"]           │    │
                    │                             │  └─────────────────────────┘    │
                    │                             └─────────────────────────────────┘
```

### Design Philosophy

- **Trustless Oracle**: No single party controls market outcomes. TxLINE's Merkle proofs provide cryptographic guarantees.
- **Permissionless Resolution**: Anyone can submit a valid Merkle proof to resolve a market.
- **Account Minimization**: Six PDA types cover all functionality with no redundant state.
- **Deterministic PDAs**: All addresses are derivable client-side without on-chain lookups.
- **Composability**: Each instruction is atomic; multi-instruction transactions can batch operations.

---

## 2. PDA Seed Design

All PDAs use `find_program_address` with the program ID. Seeds are deterministic
so clients can reconstruct addresses without querying the chain.

```
PlatformState
  seeds = ["platform"]
  Singleton global configuration. One per program deployment.

MatchRegistry
  seeds = ["match", txline_match_id.as_bytes()]
  One per unique TxLINE match. Acts as a namespace for event markets.

MarketAccount
  seeds = ["market", match_registry_pubkey.as_ref(), &event_id.to_le_bytes()]
  event_id is a sequential u32 counter stored in MatchRegistry.event_count.
  Auto-incremented on market creation. This design enables:
  - Discovery via getProgramAccounts with memcmp on match_registry field
  - Iterative scanning from event_id 0 to event_count-1

PredictionPosition
  seeds = ["prediction", market_pubkey.as_ref(), user_pubkey.as_ref()]
  One per user per market. PDA uniqueness enforces single-prediction constraint.

OracleProof
  seeds = ["oracle_proof", market_pubkey.as_ref(), txline_match_id.as_bytes()]
  Immutable record of each resolution. Stores the Merkle proof on-chain.

TreasuryVault
  seeds = ["treasury", "platform"]
  Accumulates platform fees. Can be drained by authority for protocol revenue.
```

### Rust Derivation Example

```rust
let (platform_pda, bump) = Pubkey::find_program_address(
    &[b"platform"],
    ctx.program_id,
);

let (match_pda, bump) = Pubkey::find_program_address(
    &[b"match", txline_match_id.as_bytes()],
    ctx.program_id,
);

let (market_pda, bump) = Pubkey::find_program_address(
    &[b"market", match_pda.as_ref(), &event_id.to_le_bytes()],
    ctx.program_id,
);

let (pred_pda, bump) = Pubkey::find_program_address(
    &[b"prediction", market_pda.as_ref(), user_pubkey.as_ref()],
    ctx.program_id,
);

let (proof_pda, bump) = Pubkey::find_program_address(
    &[b"oracle_proof", market_pda.as_ref(), txline_match_id.as_bytes()],
    ctx.program_id,
);

let (treasury_pda, bump) = Pubkey::find_program_address(
    &[b"treasury", b"platform"],
    ctx.program_id,
);
```

---

## 3. Account Structures

### 3.1 PlatformState (Singleton)

| Field | Type | Size | Description |
|-------|------|------|-------------|
| authority | Pubkey | 32 | Admin key for parameter updates |
| txline_program_id | Pubkey | 32 | TxLINE program to trust for CPI |
| platform_fee_bps | u16 | 2 | Fee in basis points (0-10000) |
| treasury_vault | Pubkey | 32 | TreasuryVault PDA address |
| min_stake | u64 | 8 | Minimum lamports per prediction |
| max_stake | u64 | 8 | Maximum lamports per prediction |
| market_count | u64 | 8 | Total markets created (unused currently) |
| paused | bool | 1 | Emergency pause flag |
| bump | u8 | 1 | PDA bump seed |
| _reserved | [u8; 126] | 126 | Future extensions |

Total: ~250 bytes

### 3.2 MatchRegistry

| Field | Type | Description |
|-------|------|-------------|
| txline_match_id | String (4+64) | Canonical TxLINE match identifier |
| kickoff_time | i64 | Unix timestamp of kickoff |
| prediction_deadline | i64 | Latest allowed prediction time |
| status | u8 | 0=Upcoming, 1=Live, 2=Completed, 3=Cancelled |
| home_team | String (4+4) | Three-letter home team code |
| away_team | String (4+4) | Three-letter away team code |
| event_count | u32 | Number of markets created for this match |
| bump | u8 | PDA bump seed |
| _reserved | [u8; 93] | Future extensions |

### 3.3 MarketAccount

| Field | Type | Description |
|-------|------|-------------|
| match_registry | Pubkey | Parent match |
| creator | Pubkey | Market creator |
| event_id | u32 | Sequential ID within match |
| event_type | u8 | 0=Binary, 1=PlayerAction, 2=NumericOverUnder, 3=MultiChoice |
| outcome_count | u8 | Number of possible outcomes (2-8) |
| question | String (4+200) | Human-readable question |
| outcome_labels | String (4+256) | Semicolon-delimited labels |
| stat_key | String (4+64) | TxLINE stat key for resolution |
| pools | [u64; 8] | Stake pool per outcome (lamports) |
| total_predictions | u64 | Counter of total predictions |
| deadline | i64 | When predictions close |
| status | u8 | 0=Open, 1=Closed, 2=Resolved |
| winning_outcome | u8 | Resolved outcome index (255=unresolved) |
| resolution_root | [u8; 32] | Merkle root from TxLINE |
| resolved_slot | u64 | Slot when resolved |
| fees_collected | u64 | Total fees for this market |
| bump | u8 | PDA bump seed |

### 3.4 PredictionPosition

| Field | Type | Description |
|-------|------|-------------|
| market | Pubkey | Parent market |
| user | Pubkey | Predicting user |
| outcome_index | u8 | Which outcome chosen |
| amount | u64 | Net stake (lamports) |
| claimed | bool | Whether winnings claimed |
| bump | u8 | PDA bump seed |

### 3.5 OracleProof

| Field | Type | Description |
|-------|------|-------------|
| market | Pubkey | Resolved market |
| match_registry | Pubkey | Match reference |
| txline_match_id | String | TxLINE match ID |
| stat_key | String | Stat key verified |
| resolved_value | String | Value from TxLINE |
| proof_hash | [u8; 32] | Keccak leaf hash |
| merkle_root | [u8; 32] | Verified Merkle root |
| outcome_index | u8 | Mapped outcome index |
| validated | bool | Proof valid flag |
| submission_slot | u64 | Slot of submission |
| bump | u8 | PDA bump seed |

### 3.6 TreasuryVault

| Field | Type | Description |
|-------|------|-------------|
| platform | Pubkey | Parent platform |
| total_fees | u64 | Accumulated fees |
| bump | u8 | PDA bump seed |

---

## 4. Instruction Specifications

### 4.1 initialize_platform

**Purpose:** One-time platform initialization. Sets the TxLINE program anchor,
fee structure, and stake bounds.

**Accounts:**
- `authority` (signer, writable) — Platform admin, pays rent
- `platform_state` (writable) — PlatformState PDA (init)
- `treasury_vault` (writable) — TreasuryVault PDA (init)
- `system_program`

**Args:**
- `txline_program_id: Pubkey` — TxLINE program to trust
- `platform_fee_bps: u16` — Fee in basis points (max 10000)
- `min_stake: u64` — Minimum lamports per prediction
- `max_stake: u64` — Maximum lamports per prediction

**Validation:**
1. `platform_fee_bps <= 10000` (cannot exceed 100%)
2. `min_stake > 0`
3. `max_stake >= min_stake`

---

### 4.2 create_market

**Purpose:** Create a prediction market linked to a TxLINE match and stat key.

**Accounts:**
- `creator` (signer, writable) — Pays for account creation
- `platform_state` — PlatformState PDA
- `match_registry` (writable) — MatchRegistry PDA (init_if_needed)
- `market_account` (writable) — MarketAccount PDA (init)
- `system_program`

**Args:**
- `txline_match_id: String` — Canonical TxLINE match ID
- `kickoff_time: i64` — Match kickoff timestamp
- `home_team: String` — Three-letter home team code
- `away_team: String` — Three-letter away team code
- `event_type: u8` — Market type discriminator
- `question: String` — max 200 chars
- `outcome_labels: String` — Semicolon-delimited, max 256 chars
- `stat_key: String` — TxLINE stat key, max 64 chars
- `deadline_offset: i64` — Seconds after kickoff to close

**Logic:**
1. Validate platform not paused
2. Validate string lengths
3. Parse outcome labels to count (2-8 outcomes)
4. If match_registry.event_count == 0, initialize match; otherwise verify not cancelled
5. Increment event_count, use previous value as event_id
6. Initialize market with zero pools, deadline = kickoff_time + offset
7. All arithmetic uses checked_math

---

### 4.3 place_prediction

**Purpose:** Stake SOL on a specific outcome.

**Accounts:**
- `user` (signer, writable) — Transfers SOL
- `market_account` (writable) — MarketAccount PDA
- `match_registry` — MatchRegistry PDA
- `prediction_position` (writable) — PredictionPosition PDA (init)
- `platform_state` — PlatformState PDA
- `treasury_vault` (writable) — TreasuryVault PDA
- `system_program`

**Args:**
- `outcome_index: u8` — Which outcome to predict
- `amount: u64` — Lamports to stake

**Logic:**
1. Validate market is Open, match not Cancelled, platform not paused
2. Validate outcome_index < outcome_count
3. Validate stake within [min_stake, max_stake]
4. Check deadline not passed (Clock::get()?)
5. Calculate fee = amount * fee_bps / 10000
6. net_stake = amount - fee
7. Transfer SOL from user to market via system_program::transfer
8. Update market.pools[outcome_index] += net_stake
9. Initialize PredictionPosition (PDA uniqueness enforces one-per-user)

---

### 4.4 resolve_via_txline

**Purpose:** Trustless market resolution via TxLINE CPI.

**Accounts:**
- `resolver` (signer, writable) — Anyone can trigger resolution
- `market_account` (writable) — MarketAccount PDA
- `match_registry` — MatchRegistry PDA
- `platform_state` — PlatformState PDA (provides TxLINE program ID)
- `oracle_proof` (writable) — OracleProof PDA (init, stores proof on-chain)
- `txline_state` — TxLINE on-chain state account (passed to CPI)
- `txline_authority` — TxLINE authority account (passed to CPI)
- `system_program`

**Args:**
- `resolved_value: String` — The stat value from TxLINE
- `proof: Vec<[u8; 32]>` — Merkle proof sibling hashes
- `merkle_root: [u8; 32]` — Merkle root to verify against

**CPI Flow:**
1. Validate market is Open or Closed (not already Resolved)
2. Build TxlineValidateStatArgs { match_id, stat_key, value, proof, merkle_root }
3. Build TxlineValidateStat accounts { oracle_proof, txline_state, txline_authority }
4. Call cpi_validate_stat() — builds the instruction discriminator from
   hash("global:validate_stat"), serializes args, performs CPI
5. If CPI succeeds, Merkle proof is valid → map resolved_value to outcome_index
6. Set market.status = Resolved, market.winning_outcome = outcome_index
7. Store proof metadata in OracleProof account

---

### 4.5 claim_winnings

**Purpose:** Claim proportional winnings after market resolution.

**Accounts:**
- `user` (signer, writable) — Receives winnings
- `market_account` (writable) — MarketAccount PDA (close to user)
- `prediction_position` (writable) — PredictionPosition PDA (close to user)
- `platform_state` — PlatformState PDA
- `treasury_vault` (writable) — TreasuryVault PDA
- `system_program`

**Logic:**
1. Validate market.status == Resolved
2. Validate position.claimed == false
3. Validate position.outcome_index == market.winning_outcome (user won)
4. Calculate total_pool = sum(market.pools[0..outcome_count])
5. winning_pool = market.pools[winning_outcome]
6. payout = (user_stake * total_pool) / winning_pool (u128 intermediate)
7. Transfer payout lamports from market to user
8. Both accounts closed to recover rent (Anchor close constraint)

---

## 5. TxLINE CPI Flow for validate_stat

### CPI Architecture

```
  resolve_via_txline instruction
        │
        ├── 1. Validate market state
        │
        ├── 2. Build TxlineValidateStatArgs
        │       match_id: "FIFA2026-M01-ARGvsBRA"
        │       stat_key: "match.first_goal_scorer"
        │       value: "Messi"
        │       proof: [sibling_hash_1, sibling_hash_2, ...]
        │       merkle_root: 0xabcd...
        │
        ├── 3. Perform CPI into TxLINE program
        │       │
        │       ├── Instruction discriminator: sha256("global:validate_stat")[..8]
        │       ├── Serialized args (match_id, stat_key, value, proof, merkle_root)
        │       └── Account metas: [oracle_proof, txline_state, txline_authority]
        │
        ├── 4. TxLINE validates Merkle proof
        │       ├── Compute leaf_hash = keccak256(stat_key || ":" || value)
        │       ├── Walk Merkle proof path
        │       ├── Compare computed root to on-chain root
        │       └── Return Ok(()) or Err()
        │
        ├── 5. CPI returned Ok → proof valid
        │       └── Map value to outcome_index
        │       └── Resolve market
        │       └── Store OracleProof
        │
        └── CPI returned Err → entire transaction reverts
```

### Keccak-256 Leaf Construction

TxLINE uses keccak-256 for Merkle tree hashing. The leaf hash algorithm:

```
leaf_hash = keccak256(stat_key || ":" || value)
```

Example:
```
leaf_hash = keccak256("match.first_goal_scorer:Messi")
```

Our program implements `compute_txline_leaf()` with this exact algorithm in
`txline_cpi.rs` and includes unit tests verifying the computation.

### Merkle Proof Verification (Fallback)

The program also includes a local `verify_merkle_proof()` function as a
fallback verification mechanism. It walks the proof path, ordering sibling
pairs deterministically (smaller hash first), and compares to the root.
This is supplementary to the CPI and primarily used for testing.

---

## 6. TxLINE Endpoints Used

### 6.1 SSE Stream (Real-Time Data)

```
Endpoint:  https://txline.txodds.com/api/stream?match_id={match_id}
Auth:      Bearer <JWT>
Method:    GET (SSE)
```

**Purpose:** Real-time stream of match events for constructing Merkle proofs.
Each event contains a stat_key and value that can be verified on-chain.

**Events:**
```
event: stat_update
data: {"match_id":"FIFA2026-M01","stat_key":"match.goals","value":"1"}

event: stat_update
data: {"match_id":"FIFA2026-M01","stat_key":"match.first_goal_scorer","value":"Messi"}
```

### 6.2 validate_stat CPI (On-Chain Verification)

```
Program ID:  (Deployed by TxLINE — configured in PlatformState)
Instruction: validate_stat
Discriminator: sha256("global:validate_stat")[..8]
```

**Purpose:** Receive Merkle proof, verify against on-chain root, return result.
This is the trustless settlement mechanism.

### 6.3 Auth Token Activation

```
Endpoint:  POST https://txline.txodds.com/api/token/activate
Auth:      Solana wallet NaCl signing
Body:      { signed_message, public_key }
Response:  { token: "<JWT>" }
```

**Purpose:** Obtain JWT for SSE stream authentication. The signed message
proves wallet ownership.

---

## 7. Payout Mathematics

### Proportional Payout Model

FANatic Settlement uses a proportional payout model where winners share the
entire pool (including loser stakes) in proportion to their contribution.

```
Given:
  pool[i] = total net stakes on outcome i
  total_pool = sum(pool[0..n-1])
  winning_outcome = w
  user_stake = amount staked by user on winning outcome

Payout:
  payout = (user_stake * total_pool) / pool[w]
```

**Example:**
- Alice stakes 10 SOL on "Yes" → pool[0] = 10
- Bob stakes 5 SOL on "No" → pool[1] = 5
- Charlie stakes 3 SOL on "Yes" → pool[0] = 13
- Market resolves to "Yes" → w = 0
- total_pool = 15 SOL

Alice's payout: (10 * 15) / 13 = 11.538 SOL (profit: 1.538 SOL)
Charlie's payout: (3 * 15) / 13 = 3.462 SOL (profit: 0.462 SOL)

### Overflow Protection

All arithmetic uses checked_math operations. The payout calculation uses u128
intermediate precision:

```rust
let payout = (user_stake as u128)
    .checked_mul(total_pool as u128)
    .ok_or(Overflow)?
    .checked_div(winning_pool as u128)
    .ok_or(Overflow)?;
let payout_u64: u64 = payout.try_into().map_err(|_| Overflow)?;
```

---

## 8. Security Model

### Trust Assumptions

| Component | Trust Model | Justification |
|-----------|------------|---------------|
| TxLINE validate_stat | Trusted (cryptographic) | Merkle proof verification is mathematically sound |
| TxLINE data feed | Trusted for correctness | On-chain root anchors off-chain data |
| Platform authority | Semi-trusted | Can pause platform, update params, drain treasury |
| Market creator | Untrusted | Cannot influence resolution (CPI enforces proof) |
| Resolver | Untrusted | Must provide valid Merkle proof, permissionless |

### Attack Vectors Mitigated

1. **Fake resolution**: Cannot set outcome without valid Merkle proof → CPI reverts
2. **Double claim**: `claimed` flag + account closure prevents re-claiming
3. **Overflow exploit**: All arithmetic uses `checked_*` methods
4. **Deadline bypass**: Clock sysvar enforces timestamp validation
5. **Duplicate prediction**: PDA uniqueness prevents same user predicting twice
6. **Re-entrancy**: Anchor's ownership model prevents recursive CPI
7. **Front-running resolution**: Resolution is idempotent; first valid proof wins

---

## 9. Error Codes

| Code | Name | Description |
|------|------|-------------|
| 6000 | Overflow | Arithmetic overflow detected |
| 6001 | InsufficientFunds | Not enough lamports for operation |
| 6002 | Unauthorized | Signer not authorized |
| 6003 | MarketNotOpen | Market status is not Open |
| 6004 | DeadlinePassed | Prediction deadline elapsed |
| 6005 | MarketNotResolved | Market has not been resolved yet |
| 6006 | AlreadyClaimed | Winnings already claimed |
| 6007 | InvalidOutcomeIndex | Outcome index out of range |
| 6008 | StakeOutOfRange | Stake outside [min, max] |
| 6009 | PlatformPaused | Emergency pause active |
| 6010 | TooManyOutcomes | More than 8 outcomes |
| 6011 | MatchCancelled | Match has been cancelled |
| 6012 | TxlineProofInvalid | Merkle proof validation failed |
| 6013 | MarketAlreadyResolved | Market already resolved |
| 6014 | InvalidPlatformFee | Fee exceeds 10000 bps |
| 6015 | StringTooLong | String exceeds max length |
| 6016 | NotAWinner | Predicted wrong outcome |
| 6017 | MatchAlreadyRegistered | Match ID collision |
| 6018 | ZeroStake | Stake must be > 0 |

---

## 10. Deployment Guide

### Prerequisites

```bash
# Install Solana CLI
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"

# Install Anchor
cargo install --git https://github.com/coral-xyz/anchor anchor-cli --tag v0.31.1

# Verify versions
anchor --version  # 0.31.1
solana --version  # 2.1.x
```

### Build

```bash
cd GitHub_Package/
anchor build
```

This produces:
- `target/deploy/fanatic_settlement.so` — BPF bytecode
- `target/idl/fanatic_settlement.json` — IDL for client generation
- `target/types/fanatic_settlement.ts` — TypeScript types

### Deploy to DevNet

```bash
# Configure for DevNet
solana config set --url https://api.devnet.solana.com

# Ensure wallet has SOL
solana airdrop 10

# Deploy
anchor deploy --provider.cluster devnet
```

### Initialize Platform

After deployment, call `initialize_platform` with the TxLINE DevNet program ID:

```typescript
const txlineProgramId = new PublicKey("TxLiNEcYptPcoz7JdgkTf4W9aGVSF5B2ZVztbHyLs5F");

await program.methods
  .initializePlatform({
    txlineProgramId,
    platformFeeBps: 100,  // 1%
    minStake: new BN(1_000_000),    // 0.001 SOL
    maxStake: new BN(100_000_000),  // 0.1 SOL
  })
  .accounts({ /* ... */ })
  .rpc();
```

### Run Tests

```bash
anchor test --provider.cluster localnet
```

---

## Appendix: Event Type Reference

| Type Value | Name | Example Question | Outcome Labels | Resolution |
|-----------|------|-----------------|----------------|------------|
| 0 | Binary | "Will Messi score?" | "Yes;No" | Maps "Yes"/"true"/"1" → 0, else → 1 |
| 1 | PlayerAction | "Next goal scorer?" | "Messi;Ronaldo;Neymar;Mbappe;Other" | Searches labels for match |
| 2 | NumericOverUnder | "Total corners?" | "Over 8.5;Under 8.5" | Parses threshold, compares value |
| 3 | MultiChoice | "First team to score?" | "Argentina;Brazil;No Goal" | Searches labels for match |