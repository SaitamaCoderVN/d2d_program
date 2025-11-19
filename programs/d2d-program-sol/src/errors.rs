use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Program is currently paused")]
    ProgramPaused,
    #[msg("Insufficient deposit amount")]
    InsufficientDeposit,
    #[msg("Maximum concurrent sessions exceeded")]
    MaxConcurrentSessionsExceeded,
    #[msg("Invalid session status for this operation")]
    InvalidSessionStatus,
    #[msg("Maximum retry attempts exceeded")]
    MaxRetriesExceeded,
    #[msg("Session has not expired yet")]
    SessionNotExpired,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Invalid lock period")]
    InvalidLockPeriod,
    #[msg("Inactive stake")]
    InactiveStake,
    #[msg("Insufficient stake amount")]
    InsufficientStake,
    #[msg("Stake is locked")]
    StakeLocked,
    #[msg("No rewards to claim")]
    NoRewardsToClaim,
    #[msg("Insufficient treasury funds")]
    InsufficientTreasuryFunds,
    #[msg("Invalid request ID")]
    InvalidRequestId,
    #[msg("Invalid request status")]
    InvalidRequestStatus,
    #[msg("Invalid deployment status")]
    InvalidDeploymentStatus,
    #[msg("Invalid treasury wallet")]
    InvalidTreasuryWallet,
    #[msg("Invalid ephemeral key")]
    InvalidEphemeralKey,
    #[msg("Calculation overflow")]
    CalculationOverflow,
    #[msg("Time elapsed too large")]
    TimeElapsedTooLarge,
    #[msg("Negative time elapsed - clock error detected")]
    NegativeTimeElapsed,
    #[msg("Recovered funds exceed deployment cost")]
    InvalidRecoveredFunds,
    #[msg("Lock period exceeds maximum allowed (10 years)")]
    LockPeriodTooLong,
    #[msg("Insufficient principal funds in treasury pool")]
    InsufficientPrincipalFunds,
    #[msg("Fee amount exceeds maximum allowed")]
    FeeAmountTooLarge,
    #[msg("Insufficient liquid balance for withdrawal")]
    InsufficientLiquidBalance,
    #[msg("Division by zero - total deposits is zero")]
    DivisionByZero,
    #[msg("Invalid withdrawal request")]
    InvalidWithdrawalRequest,
}
