use anchor_lang::prelude::*;

#[error_code]
pub enum FanaticError {
    /// 6000 — Arithmetic overflow detected
    #[msg("Arithmetic overflow detected in calculation")]
    Overflow,

    /// 6001 — Insufficient funds provided
    #[msg("Insufficient funds provided for this operation")]
    InsufficientFunds,

    /// 6002 — Unauthorized signer
    #[msg("Unauthorized signer for this operation")]
    Unauthorized,

    /// 6003 — Market is not open for predictions
    #[msg("Market is not open for predictions")]
    MarketNotOpen,

    /// 6004 — Prediction deadline has passed
    #[msg("Prediction deadline has passed")]
    DeadlinePassed,

    /// 6005 — Market is not yet resolved
    #[msg("Market is not yet resolved")]
    MarketNotResolved,

    /// 6006 — Winnings already claimed
    #[msg("Winnings have already been claimed for this position")]
    AlreadyClaimed,

    /// 6007 — Invalid outcome index (out of range for this market)
    #[msg("Invalid outcome index — out of range for this market")]
    InvalidOutcomeIndex,

    /// 6008 — Stake amount outside allowed range
    #[msg("Stake amount outside platform min/max bounds")]
    StakeOutOfRange,

    /// 6009 — Platform is paused for emergency maintenance
    #[msg("Platform is currently paused")]
    PlatformPaused,

    /// 6010 — Too many outcomes (max 8)
    #[msg("Too many outcomes — maximum is 8")]
    TooManyOutcomes,

    /// 6011 — Match is cancelled, cannot create markets or place predictions
    #[msg("Match has been cancelled")]
    MatchCancelled,

    /// 6012 — TxLINE proof validation failed
    #[msg("TxLINE Merkle proof validation failed")]
    TxlineProofInvalid,

    /// 6013 — Market already resolved
    #[msg("Market has already been resolved")]
    MarketAlreadyResolved,

    /// 6014 — Invalid platform fee (must be <= 10000 basis points)
    #[msg("Platform fee must not exceed 10000 basis points (100%)")]
    InvalidPlatformFee,

    /// 6015 — String too long (exceeds maximum allowed length)
    #[msg("String exceeds maximum allowed length")]
    StringTooLong,

    /// 6016 — Not a winner (predicted wrong outcome)
    #[msg("Prediction did not match winning outcome — no winnings to claim")]
    NotAWinner,

    /// 6017 — Match already registered
    #[msg("Match is already registered")]
    MatchAlreadyRegistered,

    /// 6018 — Zero stake amount is not allowed
    #[msg("Stake amount must be greater than zero")]
    ZeroStake,
}