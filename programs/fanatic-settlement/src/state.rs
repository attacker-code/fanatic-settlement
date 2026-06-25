use anchor_lang::prelude::*;

#[account]
pub struct PlatformState {
    /// Authority that can update platform parameters
    pub authority: Pubkey,
    /// TxLINE program ID for CPI calls
    pub txline_program_id: Pubkey,
    /// Platform fee in basis points (e.g., 100 = 1%)
    pub platform_fee_bps: u16,
    /// Treasury vault PDA (collects fees)
    pub treasury_vault: Pubkey,
    /// Minimum stake amount in lamports
    pub min_stake: u64,
    /// Maximum stake amount in lamports
    pub max_stake: u64,
    /// Total number of markets created (counter)
    pub market_count: u64,
    /// Whether platform is paused for emergency
    pub paused: bool,
    /// Bump seed for PDA derivation
    pub bump: u8,
    /// Reserved for future upgrades
    pub _reserved: [u8; 126],
}

impl PlatformState {
    pub const LEN: usize = 8 + 32 + 32 + 2 + 32 + 8 + 8 + 8 + 1 + 1 + 126;
}

#[account]
pub struct MatchRegistry {
    /// TxLINE match identifier (e.g., "FIFA2026-M01-ARGvsBRA")
    pub txline_match_id: String,
    /// When the match kicks off (Unix timestamp)
    pub kickoff_time: i64,
    /// When predictions close for this match
    pub prediction_deadline: i64,
    /// Match status: 0=Upcoming, 1=Live, 2=Completed, 3=Cancelled
    pub status: u8,
    /// Home team three-letter code (e.g., "ARG")
    pub home_team: String,
    /// Away team three-letter code (e.g., "BRA")
    pub away_team: String,
    /// Number of event markets created for this match
    pub event_count: u32,
    /// Bump seed
    pub bump: u8,
    /// Reserved
    pub _reserved: [u8; 93],
}

impl MatchRegistry {
    pub const LEN: usize = 8 + (4 + 64) + 8 + 8 + 1 + (4 + 4) + (4 + 4) + 4 + 1 + 93;
}

#[account]
pub struct MarketAccount {
    /// Match this market belongs to
    pub match_registry: Pubkey,
    /// Creator of this market
    pub creator: Pubkey,
    /// Sequential event ID within the match
    pub event_id: u32,
    /// Event type discriminator
    /// 0 = Binary (Yes/No), 1 = PlayerAction, 2 = NumericOverUnder, 3 = MultiChoice
    pub event_type: u8,
    /// Number of possible outcomes (max 8)
    pub outcome_count: u8,
    /// Human-readable question (max 200 chars)
    pub question: String,
    /// Outcome labels packed as semicolon-delimited (e.g., "Yes;No")
    pub outcome_labels: String,
    /// TxLINE stat key used for resolution (e.g., "match.first_goal_scorer")
    pub stat_key: String,
    /// Total pool per outcome (parallel array, up to 8 outcomes)
    pub pools: [u64; 8],
    /// Total number of predictions placed
    pub total_predictions: u64,
    /// When predictions close (Unix timestamp)
    pub deadline: i64,
    /// Market status: 0=Open, 1=Closed, 2=Resolved
    pub status: u8,
    /// Winning outcome index (255 = unresolved)
    pub winning_outcome: u8,
    /// Merkle root from TxLINE at resolution time
    pub resolution_root: [u8; 32],
    /// Slot when resolved
    pub resolved_slot: u64,
    /// Total fees collected for this market
    pub fees_collected: u64,
    /// Bump seed
    pub bump: u8,
    /// Reserved
    pub _reserved: [u8; 62],
}

impl MarketAccount {
    pub const LEN: usize = 8 + 32 + 32 + 4 + 1 + 1 + (4 + 200) + (4 + 256) + (4 + 64) + 64 + 8 + 8 + 1 + 1 + 32 + 8 + 8 + 1 + 62;
}

#[account]
pub struct PredictionPosition {
    /// The market this prediction belongs to
    pub market: Pubkey,
    /// User who made the prediction
    pub user: Pubkey,
    /// Which outcome index the user chose
    pub outcome_index: u8,
    /// Amount staked in lamports
    pub amount: u64,
    /// Whether winnings have been claimed
    pub claimed: bool,
    /// Bump seed
    pub bump: u8,
    /// Reserved
    pub _reserved: [u8; 45],
}

impl PredictionPosition {
    pub const LEN: usize = 8 + 32 + 32 + 1 + 8 + 1 + 1 + 45;
}

#[account]
pub struct OracleProof {
    /// The market this proof resolves
    pub market: Pubkey,
    /// The match this proof references
    pub match_registry: Pubkey,
    /// TxLINE match identifier
    pub txline_match_id: String,
    /// The stat key verified
    pub stat_key: String,
    /// The resolved value from TxLINE
    pub resolved_value: String,
    /// The Merkle proof hash (leaf hash)
    pub proof_hash: [u8; 32],
    /// The Merkle root from TxLINE
    pub merkle_root: [u8; 32],
    /// Which outcome index this value maps to
    pub outcome_index: u8,
    /// Whether this proof has been validated
    pub validated: bool,
    /// Slot when proof was submitted
    pub submission_slot: u64,
    /// Bump seed
    pub bump: u8,
    /// Reserved
    pub _reserved: [u8; 85],
}

impl OracleProof {
    pub const LEN: usize = 8 + 32 + 32 + (4 + 64) + (4 + 64) + (4 + 128) + 32 + 32 + 1 + 1 + 8 + 1 + 85;
}

#[account]
pub struct TreasuryVault {
    /// Platform this vault belongs to
    pub platform: Pubkey,
    /// Total fees accumulated
    pub total_fees: u64,
    /// Bump seed
    pub bump: u8,
    /// Reserved
    pub _reserved: [u8; 119],
}

impl TreasuryVault {
    pub const LEN: usize = 8 + 32 + 8 + 1 + 119;
}

/// Event types for prediction markets
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Binary = 0,
    PlayerAction = 1,
    NumericOverUnder = 2,
    MultiChoice = 3,
}

/// Match status values
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum MatchStatus {
    Upcoming = 0,
    Live = 1,
    Completed = 2,
    Cancelled = 3,
}

/// Market status values
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum MarketStatus {
    Open = 0,
    Closed = 1,
    Resolved = 2,
}