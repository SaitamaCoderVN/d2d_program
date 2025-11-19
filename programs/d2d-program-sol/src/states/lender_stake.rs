use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

/// Backer's deposit position in the pool
/// 
/// Reward-per-share model:
/// - deposited_amount: Amount of SOL deposited (net after fees)
/// - reward_debt: Tracks accumulated rewards at deposit time (deposited_amount * reward_per_share)
/// - claimed_total: Total rewards claimed so far
#[account]
#[derive(InitSpace)]
pub struct BackerDeposit {
    pub backer: Pubkey,          // Backer public key
    pub deposited_amount: u64,   // Amount of SOL deposited (lamports, net after fees)
    pub reward_debt: u128,        // Reward debt (deposited_amount * reward_per_share at deposit)
    pub claimed_total: u64,      // Total rewards claimed so far (lamports)
    pub is_active: bool,         // Is deposit active
    pub bump: u8,                // PDA bump
}

/// Legacy alias for backward compatibility
pub type LenderStake = BackerDeposit;

impl BackerDeposit {
    pub const PREFIX_SEED: &'static [u8] = b"lender_stake"; // Keep same seed for backward compatibility

    /// Calculate claimable rewards using reward-per-share
    /// Formula: (deposited_amount * reward_per_share - reward_debt) / PRECISION
    pub fn calculate_claimable_rewards(&self, reward_per_share: u128) -> Result<u64> {
        use crate::states::TreasuryPool;
        
        let accumulated = (self.deposited_amount as u128)
            .checked_mul(reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        let claimable = accumulated
            .checked_sub(self.reward_debt)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(TreasuryPool::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        Ok(claimable as u64)
    }

    /// Update reward_debt after deposit or claim
    /// Sets reward_debt = deposited_amount * reward_per_share
    pub fn update_reward_debt(&mut self, reward_per_share: u128) -> Result<()> {
        self.reward_debt = (self.deposited_amount as u128)
            .checked_mul(reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(())
    }
}
