# FANatic Settlement

**Trustless World Cup Prediction Markets on Solana — TxODDS World Cup Hackathon**

## Live on DevNet

**Program ID**: `2Ju8T8v7QkSXxTpfxTQhfJNKW1DwGU8Eq4dBzz8DANoG`

[View on Solana Explorer](https://explorer.solana.com/address/2Ju8T8v7QkSXxTpfxTQhfJNKW1DwGU8Eq4dBzz8DANoG?cluster=devnet)

## What It Does

FANatic Settlement is a Solana Anchor program that enables trustless prediction markets for the 2026 FIFA World Cup. Unlike traditional prediction markets where a centralized entity resolves outcomes, FANatic Settlement uses **cryptographic Merkle proofs** from the TxLINE protocol, verified on-chain via Cross-Program Invocation (CPI).

**No trusted oracle. No market creator authority. Just math.**

## Architecture

```
5 Instructions:
├── initialize_platform   — Set TxLINE as the trust anchor
├── create_market         — Create a prediction market for any World Cup match
├── place_prediction      — Stake SOL on a predicted outcome
├── resolve_via_txline    — CPI into TxLINE's validate_stat with Merkle proof
└── claim_winnings        — Proportional payout (winners share the full pool)

6 PDAs:
├── PlatformState         — Platform configuration
├── MatchRegistry         — Links to TxLINE match data
├── MarketAccount         — Prediction market state (up to 8 outcomes)
├── PredictionPosition    — Per-user prediction record
├── OracleProof           — Immutable on-chain proof record
└── TreasuryVault         — Fee collection
```

## Trustless Settlement Flow

1. TxLINE streams World Cup match data via SSE with cryptographic Merkle proofs
2. Anyone can construct a Merkle proof from the stream and submit it on-chain
3. FANatic Settlement calls TxLINE's `validate_stat` instruction via CPI
4. TxLINE recomputes the Merkle root — if it matches, the proof is valid
5. Market resolves automatically based on the cryptographically verified outcome
6. Winners claim proportional payouts from the combined pool

## Quick Start

```bash
# Install dependencies
npm install

# Build the program
anchor build

# Run integration tests (requires solana-test-validator)
anchor test

# Deploy to DevNet
solana config set --url devnet
anchor deploy
```

## Tech Stack

- **Solana** — Settlement layer (~400ms finality, sub-cent fees)
- **Anchor 0.31** — Framework for Solana program development
- **TxLINE** — Real-time sports data with cryptographic Merkle proofs
- **Rust** — Zero-overhead abstractions, checked math throughout

## Track

**Prediction Markets and Settlement** — TxODDS World Cup Hackathon (Summer 2026)

Prize Pool: $18,000 USDT

## Submission

- **Hackathon**: [World Cup Hackathon on Superteam Earn](https://superteam.fun/earn/hackathon/world-cup)
- **Agent ID**: `142c2f6f-e91c-4588-9ffc-c94852923818`

## License

MIT
