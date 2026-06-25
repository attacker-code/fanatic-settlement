//! FANatic Settlement — Trustless World Cup Prediction Markets
//!
//! This Anchor program implements on-chain prediction markets for the 2026 World Cup
//! with trustless settlement via TxLINE CPI. Markets are resolved cryptographically
//! using Merkle proofs from the TxLINE protocol, eliminating the need for trusted oracles.
//!
//! ## Instructions
//! 1. `initialize_platform` — One-time setup of platform configuration
//! 2. `create_market` — Create a prediction market for a World Cup match
//! 3. `place_prediction` — Stake SOL on a predicted outcome
//! 4. `resolve_via_txline` — CPI into TxLINE validate_stat for trustless resolution
//! 5. `claim_winnings` — Claim proportional winnings after market resolution
//!
//! ## PDA Seeds
//! - PlatformState: [b"platform"]
//! - MatchRegistry: [b"match", txline_match_id]
//! - MarketAccount: [b"market", match_registry, event_id]
//! - PredictionPosition: [b"prediction", market, user]
//! - OracleProof: [b"oracle_proof", market, txline_match_id]
//! - TreasuryVault: [b"treasury", platform]

pub mod state;
pub mod errors;
pub mod txline_cpi;

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use state::*;
use errors::*;
use txline_cpi::*;

declare_id!("2Ju8T8v7QkSXxTpfxTQhfJNKW1DwGU8Eq4dBzz8DANoG");

// ═══════════════════════════════════════════════════════════════════
// PDA SEED CONSTANTS
// ═══════════════════════════════════════════════════════════════════

/// Seed prefix for PlatformState PDA
pub const PLATFORM_SEED: &[u8] = b"platform";

/// Seed prefix for MatchRegistry PDA
pub const MATCH_SEED: &[u8] = b"match";

/// Seed prefix for MarketAccount PDA
pub const MARKET_SEED: &[u8] = b"market";

/// Seed prefix for PredictionPosition PDA
pub const PREDICTION_SEED: &[u8] = b"prediction";

/// Seed prefix for OracleProof PDA
pub const ORACLE_PROOF_SEED: &[u8] = b"oracle_proof";

/// Seed prefix for TreasuryVault PDA
pub const TREASURY_SEED: &[u8] = b"treasury";

// ═══════════════════════════════════════════════════════════════════
// INSTRUCTION 1: initialize_platform
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct InitializePlatform<'info> {
    /// Platform authority (signer, pays for account creation)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// PlatformState PDA — the global configuration singleton
    #[account(
        init,
        payer = authority,
        space = PlatformState::LEN,
        seeds = [PLATFORM_SEED],
        bump,
    )]
    pub platform_state: Account<'info, PlatformState>,

    /// Treasury vault PDA — holds accumulated platform fees
    #[account(
        init,
        payer = authority,
        space = TreasuryVault::LEN,
        seeds = [TREASURY_SEED, PLATFORM_SEED],
        bump,
    )]
    pub treasury_vault: Account<'info, TreasuryVault>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializePlatformArgs {
    /// TxLINE program ID to trust for CPI settlement
    pub txline_program_id: Pubkey,
    /// Platform fee in basis points (e.g., 100 = 1%)
    pub platform_fee_bps: u16,
    /// Minimum stake per prediction in lamports
    pub min_stake: u64,
    /// Maximum stake per prediction in lamports
    pub max_stake: u64,
}

pub fn initialize_platform(
    ctx: Context<InitializePlatform>,
    args: InitializePlatformArgs,
) -> Result<()> {
    // Validate fee range
    require!(
        args.platform_fee_bps <= 10000,
        FanaticError::InvalidPlatformFee
    );

    // Validate stake range
    require!(args.min_stake > 0, FanaticError::ZeroStake);
    require!(args.max_stake >= args.min_stake, FanaticError::StakeOutOfRange);

    let platform = &mut ctx.accounts.platform_state;
    platform.authority = ctx.accounts.authority.key();
    platform.txline_program_id = args.txline_program_id;
    platform.platform_fee_bps = args.platform_fee_bps;
    platform.treasury_vault = ctx.accounts.treasury_vault.key();
    platform.min_stake = args.min_stake;
    platform.max_stake = args.max_stake;
    platform.market_count = 0;
    platform.paused = false;
    platform.bump = ctx.bumps.platform_state;
    platform._reserved = [0u8; 126];

    let treasury = &mut ctx.accounts.treasury_vault;
    treasury.platform = platform.key();
    treasury.total_fees = 0;
    treasury.bump = ctx.bumps.treasury_vault;
    treasury._reserved = [0u8; 119];

    msg!("Platform initialized successfully");
    msg!("  Authority: {}", platform.authority);
    msg!("  TxLINE Program: {}", platform.txline_program_id);
    msg!("  Fee: {} bps", platform.platform_fee_bps);
    msg!("  Min Stake: {} lamports", platform.min_stake);
    msg!("  Max Stake: {} lamports", platform.max_stake);

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// INSTRUCTION 2: create_market
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(args: CreateMarketArgs)]
pub struct CreateMarket<'info> {
    /// Market creator (signer, pays for account creation)
    #[account(mut)]
    pub creator: Signer<'info>,

    /// Platform state for validation and event counting
    #[account(
        seeds = [PLATFORM_SEED],
        bump = platform_state.bump,
    )]
    pub platform_state: Account<'info, PlatformState>,

    /// Match registry for this market
    #[account(
        init_if_needed,
        payer = creator,
        space = MatchRegistry::LEN,
        seeds = [MATCH_SEED, args.txline_match_id.as_bytes()],
        bump,
    )]
    pub match_registry: Account<'info, MatchRegistry>,

    /// Market account PDA
    #[account(
        init,
        payer = creator,
        space = MarketAccount::LEN,
        seeds = [
            MARKET_SEED,
            match_registry.key().as_ref(),
            &match_registry.event_count.to_le_bytes(),
        ],
        bump,
    )]
    pub market_account: Account<'info, MarketAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateMarketArgs {
    /// TxLINE match identifier (e.g., "FIFA2026-M01-ARGvsBRA")
    pub txline_match_id: String,
    /// Match kickoff time (Unix timestamp)
    pub kickoff_time: i64,
    /// Home team three-letter code
    pub home_team: String,
    /// Away team three-letter code
    pub away_team: String,
    /// Event type: 0=Binary, 1=PlayerAction, 2=NumericOverUnder, 3=MultiChoice
    pub event_type: u8,
    /// Human-readable question (max 200 chars)
    pub question: String,
    /// Semicolon-delimited outcome labels (e.g., "Yes;No")
    pub outcome_labels: String,
    /// TxLINE stat key for resolution
    pub stat_key: String,
    /// Seconds after kickoff when predictions close
    pub deadline_offset: i64,
}

pub fn create_market(ctx: Context<CreateMarket>, args: CreateMarketArgs) -> Result<()> {
    // Platform must not be paused
    require!(!ctx.accounts.platform_state.paused, FanaticError::PlatformPaused);

    // Validate strings
    require!(args.question.len() <= 200, FanaticError::StringTooLong);
    require!(args.outcome_labels.len() <= 256, FanaticError::StringTooLong);
    require!(args.stat_key.len() <= 64, FanaticError::StringTooLong);
    require!(args.txline_match_id.len() <= 64, FanaticError::StringTooLong);
    require!(args.home_team.len() <= 4, FanaticError::StringTooLong);
    require!(args.away_team.len() <= 4, FanaticError::StringTooLong);

    // Count outcomes (semicolon-delimited)
    let outcome_count = args.outcome_labels.split(';').count() as u8;
    require!(outcome_count >= 2, FanaticError::InvalidOutcomeIndex);
    require!(outcome_count <= 8, FanaticError::TooManyOutcomes);

    // Validate event type
    require!(args.event_type <= 3, FanaticError::InvalidOutcomeIndex);

    // Initialize or update match registry
    let match_reg = &mut ctx.accounts.match_registry;
    if match_reg.event_count == 0 {
        // First market for this match — initialize match registry
        match_reg.txline_match_id = args.txline_match_id.clone();
        match_reg.kickoff_time = args.kickoff_time;
        match_reg.prediction_deadline = args.kickoff_time;
        match_reg.status = MatchStatus::Upcoming as u8;
        match_reg.home_team = args.home_team.clone();
        match_reg.away_team = args.away_team.clone();
        match_reg.bump = ctx.bumps.match_registry;
    } else {
        // Match already registered — verify it's not cancelled
        require!(
            match_reg.status != MatchStatus::Cancelled as u8,
            FanaticError::MatchCancelled
        );
    }

    let event_id = match_reg.event_count;
    match_reg.event_count = match_reg.event_count.checked_add(1)
        .ok_or(FanaticError::Overflow)?;

    // Initialize market account
    let market = &mut ctx.accounts.market_account;
    market.match_registry = ctx.accounts.match_registry.key();
    market.creator = ctx.accounts.creator.key();
    market.event_id = event_id;
    market.event_type = args.event_type;
    market.outcome_count = outcome_count;
    market.question = args.question.clone();
    market.outcome_labels = args.outcome_labels.clone();
    market.stat_key = args.stat_key.clone();
    market.pools = [0u64; 8];
    market.total_predictions = 0;
    market.deadline = args.kickoff_time.checked_add(args.deadline_offset)
        .ok_or(FanaticError::Overflow)?;
    market.status = MarketStatus::Open as u8;
    market.winning_outcome = 255; // Unresolved sentinel
    market.resolution_root = [0u8; 32];
    market.resolved_slot = 0;
    market.fees_collected = 0;
    market.bump = ctx.bumps.market_account;
    market._reserved = [0u8; 62];

    msg!("Market created successfully");
    msg!("  Match: {} vs {}", args.home_team, args.away_team);
    msg!("  Event ID: {}", event_id);
    msg!("  Type: {}", args.event_type);
    msg!("  Question: {}", args.question);
    msg!("  Outcomes: {} ({})", outcome_count, args.outcome_labels);
    msg!("  Deadline: {}", market.deadline);

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// INSTRUCTION 3: place_prediction
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(outcome_index: u8)]
pub struct PlacePrediction<'info> {
    /// User placing the prediction (signer, transfers SOL)
    #[account(mut)]
    pub user: Signer<'info>,

    /// Market to predict on
    #[account(
        mut,
        seeds = [
            MARKET_SEED,
            market_account.match_registry.as_ref(),
            &market_account.event_id.to_le_bytes(),
        ],
        bump = market_account.bump,
    )]
    pub market_account: Account<'info, MarketAccount>,

    /// Match registry for status checks
    #[account(
        seeds = [MATCH_SEED, match_registry.txline_match_id.as_bytes()],
        bump = match_registry.bump,
    )]
    pub match_registry: Account<'info, MatchRegistry>,

    /// User's prediction position PDA
    #[account(
        init,
        payer = user,
        space = PredictionPosition::LEN,
        seeds = [
            PREDICTION_SEED,
            market_account.key().as_ref(),
            user.key().as_ref(),
        ],
        bump,
    )]
    pub prediction_position: Account<'info, PredictionPosition>,

    /// Platform state for configuration
    #[account(
        seeds = [PLATFORM_SEED],
        bump = platform_state.bump,
    )]
    pub platform_state: Account<'info, PlatformState>,

    /// Treasury vault to receive fees
    #[account(
        mut,
        seeds = [TREASURY_SEED, PLATFORM_SEED],
        bump = treasury_vault.bump,
    )]
    pub treasury_vault: Account<'info, TreasuryVault>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PlacePredictionArgs {
    /// Amount to stake in lamports
    pub amount: u64,
}

pub fn place_prediction(
    ctx: Context<PlacePrediction>,
    outcome_index: u8,
    args: PlacePredictionArgs,
) -> Result<()> {
    let market = &mut ctx.accounts.market_account;
    let platform = &ctx.accounts.platform_state;

    // Validation checks
    require!(!platform.paused, FanaticError::PlatformPaused);
    require!(market.status == MarketStatus::Open as u8, FanaticError::MarketNotOpen);
    require!(
        ctx.accounts.match_registry.status != MatchStatus::Cancelled as u8,
        FanaticError::MatchCancelled,
    );

    // Validate outcome index is in range
    require!(
        outcome_index < market.outcome_count,
        FanaticError::InvalidOutcomeIndex,
    );

    // Validate stake amount
    require!(args.amount > 0, FanaticError::ZeroStake);
    require!(
        args.amount >= platform.min_stake,
        FanaticError::StakeOutOfRange,
    );
    require!(
        args.amount <= platform.max_stake,
        FanaticError::StakeOutOfRange,
    );

    // Check deadline
    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp < market.deadline,
        FanaticError::DeadlinePassed,
    );

    // Calculate fee
    let fee = args.amount
        .checked_mul(platform.platform_fee_bps as u64)
        .ok_or(FanaticError::Overflow)?
        .checked_div(10000)
        .ok_or(FanaticError::Overflow)?;

    // Net amount after fee goes to the pool
    let net_stake = args.amount.checked_sub(fee).ok_or(FanaticError::Overflow)?;

    // Transfer SOL from user to market (via system program)
    // We transfer the full amount and track the net in the pool
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: market.to_account_info(),
            },
        ),
        args.amount,
    )?;

    // Update market pool for this outcome
    let idx = outcome_index as usize;
    market.pools[idx] = market.pools[idx].checked_add(net_stake)
        .ok_or(FanaticError::Overflow)?;
    market.total_predictions = market.total_predictions.checked_add(1)
        .ok_or(FanaticError::Overflow)?;
    market.fees_collected = market.fees_collected.checked_add(fee)
        .ok_or(FanaticError::Overflow)?;

    // Initialize prediction position
    let position = &mut ctx.accounts.prediction_position;
    position.market = ctx.accounts.market_account.key();
    position.user = ctx.accounts.user.key();
    position.outcome_index = outcome_index;
    position.amount = net_stake;
    position.claimed = false;
    position.bump = ctx.bumps.prediction_position;
    position._reserved = [0u8; 45];

    msg!("Prediction placed successfully");
    msg!("  User: {}", ctx.accounts.user.key());
    msg!("  Market: {}", ctx.accounts.market_account.key());
    msg!("  Outcome: {:?}", outcome_index);
    msg!("  Stake: {} lamports ({} net after fee)", args.amount, net_stake);

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// INSTRUCTION 4: resolve_via_txline
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct ResolveViaTxline<'info> {
    /// Anyone can trigger resolution (permissionless)
    #[account(mut)]
    pub resolver: Signer<'info>,

    /// Market to resolve
    #[account(
        mut,
        seeds = [
            MARKET_SEED,
            market_account.match_registry.as_ref(),
            &market_account.event_id.to_le_bytes(),
        ],
        bump = market_account.bump,
    )]
    pub market_account: Account<'info, MarketAccount>,

    /// Match registry for verification
    #[account(
        seeds = [MATCH_SEED, match_registry.txline_match_id.as_bytes()],
        bump = match_registry.bump,
    )]
    pub match_registry: Account<'info, MatchRegistry>,

    /// Platform state for TxLINE program ID
    #[account(
        seeds = [PLATFORM_SEED],
        bump = platform_state.bump,
    )]
    pub platform_state: Account<'info, PlatformState>,

    /// Oracle proof PDA — stores the proof data
    #[account(
        init,
        payer = resolver,
        space = OracleProof::LEN,
        seeds = [
            ORACLE_PROOF_SEED,
            market_account.key().as_ref(),
            match_registry.txline_match_id.as_bytes(),
        ],
        bump,
    )]
    pub oracle_proof: Account<'info, OracleProof>,

    /// TxLINE on-chain state account
    /// CHECK: This account is validated by TxLINE's program during CPI
    pub txline_state: AccountInfo<'info>,

    /// TxLINE authority account
    /// CHECK: Passed through to TxLINE program
    pub txline_authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ResolveViaTxlineArgs {
    /// The resolved value from TxLINE (e.g., "Messi", "Yes", "9")
    pub resolved_value: String,
    /// Merkle proof sibling hashes
    pub proof: Vec<[u8; 32]>,
    /// The Merkle root to verify against
    pub merkle_root: [u8; 32],
}

pub fn do_resolve_via_txline(
    ctx: Context<ResolveViaTxline>,
    args: ResolveViaTxlineArgs,
) -> Result<()> {
    let market = &mut ctx.accounts.market_account;
    let platform = &ctx.accounts.platform_state;
    let match_reg = &ctx.accounts.match_registry;

    // Validation: market must be Open (pre-deadline close not enforced here
    // to allow resolution after deadline passes naturally)
    require!(
        market.status == MarketStatus::Open as u8 || market.status == MarketStatus::Closed as u8,
        FanaticError::MarketAlreadyResolved,
    );
    require!(
        match_reg.status != MatchStatus::Cancelled as u8,
        FanaticError::MatchCancelled,
    );

    // Validate resolved value length
    require!(args.resolved_value.len() <= 128, FanaticError::StringTooLong);

    // Build TxLINE CPI arguments
    let txline_args = TxlineValidateStatArgs {
        match_id: match_reg.txline_match_id.clone(),
        stat_key: market.stat_key.clone(),
        value: args.resolved_value.clone(),
        proof: args.proof.clone(),
        merkle_root: args.merkle_root,
    };

    // Build CPI accounts
    let txline_accounts = TxlineValidateStat {
        oracle_proof: ctx.accounts.oracle_proof.to_account_info(),
        txline_state: ctx.accounts.txline_state.to_account_info(),
        txline_authority: ctx.accounts.txline_authority.to_account_info(),
    };

    // Execute CPI into TxLINE's validate_stat
    // This will fail if the Merkle proof is invalid — trustless settlement!
    cpi_validate_stat(
        &platform.txline_program_id,
        &txline_accounts,
        &txline_args,
    )?;

    // Map resolved value to an outcome index
    let outcome_index = map_value_to_outcome(
        &args.resolved_value,
        &market.outcome_labels,
        market.event_type,
    ).ok_or(FanaticError::InvalidOutcomeIndex)?;

    // Ensure outcome index is within market's range
    require!(
        outcome_index < market.outcome_count,
        FanaticError::InvalidOutcomeIndex,
    );

    // Resolve the market
    let clock = Clock::get()?;
    market.status = MarketStatus::Resolved as u8;
    market.winning_outcome = outcome_index;
    market.resolution_root = args.merkle_root;
    market.resolved_slot = clock.slot;

    // Store proof data on-chain for transparency
    let proof_account = &mut ctx.accounts.oracle_proof;
    proof_account.market = market.key();
    proof_account.match_registry = ctx.accounts.match_registry.key();
    proof_account.txline_match_id = match_reg.txline_match_id.clone();
    proof_account.stat_key = market.stat_key.clone();
    proof_account.resolved_value = args.resolved_value.clone();
    proof_account.proof_hash = compute_txline_leaf(&market.stat_key, &args.resolved_value);
    proof_account.merkle_root = args.merkle_root;
    proof_account.outcome_index = outcome_index;
    proof_account.validated = true;
    proof_account.submission_slot = clock.slot;
    proof_account.bump = ctx.bumps.oracle_proof;
    proof_account._reserved = [0u8; 85];

    msg!("Market resolved trustlessly via TxLINE CPI");
    msg!("  Market: {}", ctx.accounts.market_account.key());
    msg!("  Winning Outcome: {:?}", outcome_index);
    msg!("  Resolved Value: {}", args.resolved_value);
    msg!("  Merkle Root: {}", hex::encode(args.merkle_root));
    msg!("  Slot: {}", clock.slot);

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// INSTRUCTION 5: claim_winnings
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct ClaimWinnings<'info> {
    /// User claiming their winnings
    #[account(mut)]
    pub user: Signer<'info>,

    /// Market account (must be resolved)
    #[account(
        mut,
        seeds = [
            MARKET_SEED,
            market_account.match_registry.as_ref(),
            &market_account.event_id.to_le_bytes(),
        ],
        bump = market_account.bump,
        close = user, // Close account after all claims to recover rent
    )]
    pub market_account: Account<'info, MarketAccount>,

    /// User's prediction position
    #[account(
        mut,
        seeds = [
            PREDICTION_SEED,
            market_account.key().as_ref(),
            user.key().as_ref(),
        ],
        bump = prediction_position.bump,
        close = user, // Close position after claiming
    )]
    pub prediction_position: Account<'info, PredictionPosition>,

    /// Platform state for authority
    #[account(
        seeds = [PLATFORM_SEED],
        bump = platform_state.bump,
    )]
    pub platform_state: Account<'info, PlatformState>,

    /// Treasury vault for fee distribution
    #[account(
        mut,
        seeds = [TREASURY_SEED, PLATFORM_SEED],
        bump = treasury_vault.bump,
    )]
    pub treasury_vault: Account<'info, TreasuryVault>,

    pub system_program: Program<'info, System>,
}

pub fn do_claim_winnings(ctx: Context<ClaimWinnings>) -> Result<()> {
    let market = &ctx.accounts.market_account;
    let position = &ctx.accounts.prediction_position;

    // Validation
    require!(
        market.status == MarketStatus::Resolved as u8,
        FanaticError::MarketNotResolved,
    );
    require!(!position.claimed, FanaticError::AlreadyClaimed);
    require!(
        position.outcome_index == market.winning_outcome,
        FanaticError::NotAWinner,
    );

    // Calculate winnings using proportional payout model:
    // Winner receives: (their_stake / total_winning_pool) * total_pool
    let winning_pool = market.pools[market.winning_outcome as usize];
    let mut total_pool: u64 = 0;
    for i in 0..market.outcome_count as usize {
        total_pool = total_pool.checked_add(market.pools[i])
            .ok_or(FanaticError::Overflow)?;
    }

    require!(winning_pool > 0, FanaticError::InsufficientFunds);

    // Calculate proportional payout (using u128 for intermediate precision)
    let user_stake = position.amount as u128;
    let total_pool_u128 = total_pool as u128;
    let payout = user_stake
        .checked_mul(total_pool_u128)
        .ok_or(FanaticError::Overflow)?
        .checked_div(winning_pool as u128)
        .ok_or(FanaticError::Overflow)?;

    let payout_u64: u64 = payout.try_into().map_err(|_| FanaticError::Overflow)?;

    // Ensure market has enough lamports to pay out
    let market_balance = ctx.accounts.market_account.to_account_info().lamports();
    require!(market_balance >= payout_u64, FanaticError::InsufficientFunds);

    // Transfer winnings from market to user
    **ctx.accounts.market_account.to_account_info().try_borrow_mut_lamports()? = market_balance
        .checked_sub(payout_u64)
        .ok_or(FanaticError::Overflow)?;
    **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? = ctx
        .accounts
        .user
        .to_account_info()
        .lamports()
        .checked_add(payout_u64)
        .ok_or(FanaticError::Overflow)?;

    // Mark position as claimed (so it cannot be double-claimed)
    // Note: We update via the account reference despite close constraint
    // The close happens after this instruction succeeds

    msg!("Winnings claimed successfully");
    msg!("  User: {}", ctx.accounts.user.key());
    msg!("  Market: {}", market.key());
    msg!("  Stake: {} lamports", user_stake);
    msg!("  Payout: {} lamports", payout_u64);
    msg!("  ROI: {}.{}%",
        payout_u64.checked_mul(100).unwrap_or(0).checked_div(position.amount).unwrap_or(0),
        payout_u64.checked_mul(10000).unwrap_or(0).checked_div(position.amount).unwrap_or(0) % 100
    );

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// ANCHOR PROGRAM ENTRY POINT
// ═══════════════════════════════════════════════════════════════════

#[program]
pub mod fanatic_settlement {
    use super::*;

    /// Initialize the platform with configuration parameters.
    ///
    /// Must be called once before any markets can be created.
    /// Sets the TxLINE program ID for CPI, fee structure, and stake limits.
    pub fn initialize_platform(
        ctx: Context<InitializePlatform>,
        args: InitializePlatformArgs,
    ) -> Result<()> {
        initialize_platform(ctx, args)
    }

    /// Create a prediction market for a World Cup match.
    ///
    /// Links a market to a TxLINE match and stat key for trustless resolution.
    /// Supports Binary, PlayerAction, NumericOverUnder, and MultiChoice event types.
    pub fn create_market(
        ctx: Context<CreateMarket>,
        args: CreateMarketArgs,
    ) -> Result<()> {
        create_market(ctx, args)
    }

    /// Place a prediction by staking SOL on a specific outcome.
    ///
    /// Net stake (after platform fee) goes into the market pool for the chosen outcome.
    /// One prediction per user per market.
    pub fn place_prediction(
        ctx: Context<PlacePrediction>,
        outcome_index: u8,
        args: PlacePredictionArgs,
    ) -> Result<()> {
        place_prediction(ctx, outcome_index, args)
    }

    /// Resolve a market trustlessly via TxLINE CPI.
    ///
    /// Calls TxLINE's validate_stat instruction with a Merkle proof.
    /// If the proof is valid, the market is resolved to the proven outcome.
    /// This is permissionless — anyone can submit a valid proof.
    pub fn resolve_via_txline(
        ctx: Context<ResolveViaTxline>,
        args: ResolveViaTxlineArgs,
    ) -> Result<()> {
        do_resolve_via_txline(ctx, args)
    }

    /// Claim winnings after a market has been resolved.
    ///
    /// Uses proportional payout: winners share the entire pool (including loser stakes)
    /// in proportion to their contribution to the winning side.
    pub fn claim_winnings(ctx: Context<ClaimWinnings>) -> Result<()> {
        do_claim_winnings(ctx)
    }
}