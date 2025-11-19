use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

/// Fee-Based Treasury System with Reward-Per-Share Model
/// 
/// Efficient reward distribution using accumulator pattern:
/// - reward_per_share: Accumulator that increases when fees are credited
/// - Each backer tracks reward_debt = deposited_amount * reward_per_share at deposit time
/// - Claimable = (deposited_amount * reward_per_share - reward_debt) / PRECISION
#[account]
#[derive(InitSpace)]
pub struct TreasuryPool {
    // Reward-per-share tracking
    pub reward_per_share: u128,            // Accumulator for rewards (scaled by PRECISION)
    pub total_deposited: u64,              // Total SOL deposited by all backers (lamports)
    pub liquid_balance: u64,                // Available balance for withdrawals (lamports)
    
    // Pool balances
    pub reward_pool_balance: u64,           // Total rewards available (from fees)
    pub platform_pool_balance: u64,         // Platform fees (from 0.1% fees)
    
    // Fee rates (in basis points: 100 = 1%)
    pub reward_fee_bps: u64,                // Reward fee: 100 bps = 1%
    pub platform_fee_bps: u64,              // Platform fee: 10 bps = 0.1%
    
    // Admin and control
    pub admin: Pubkey,                      // Admin public key
    pub dev_wallet: Pubkey,                 // Dev wallet that receives deposits for deployments
    pub emergency_pause: bool,               // Emergency pause flag
    
    // PDA bumps
    pub reward_pool_bump: u8,               // Bump for Reward Pool PDA
    pub platform_pool_bump: u8,             // Bump for Platform Pool PDA
    pub bump: u8,                           // Bump for TreasuryPool PDA
    
    // Legacy fields for backward compatibility (deprecated)
    pub backer_total_staked: u128,         // DEPRECATED
    pub backer_stake_pool_bump: u8,        // DEPRECATED
    pub total_rewards_distributed: u128,   // DEPRECATED
    pub admin_pool_balance: u128,          // DEPRECATED
    pub admin_pool_bump: u8,               // DEPRECATED
    pub current_apy_bps: u64,              // DEPRECATED
    pub last_apy_update_ts: i64,           // DEPRECATED
    pub last_distribution_time: i64,        // DEPRECATED
    pub total_staked: u64,                 // DEPRECATED
    pub total_fees_collected: u64,         // DEPRECATED
    pub current_apy: u64,                  // DEPRECATED
    pub treasury_wallet: Pubkey,           // DEPRECATED
}

impl TreasuryPool {
    pub const PREFIX_SEED: &'static [u8] = b"treasury_pool";
    pub const REWARD_POOL_SEED: &'static [u8] = b"reward_pool";
    pub const PLATFORM_POOL_SEED: &'static [u8] = b"platform_pool";
    
    // Legacy constants for backward compatibility
    pub const ADMIN_POOL_SEED: &'static [u8] = b"platform_pool"; // Maps to platform_pool
    pub const MAX_FEE_AMOUNT: u128 = 1_000_000_000 * 1_000_000_000; // Legacy alias
    
    // Fee rates (fixed)
    pub const REWARD_FEE_BPS: u64 = 100;      // 1% = 100 basis points
    pub const PLATFORM_FEE_BPS: u64 = 10;     // 0.1% = 10 basis points
    
    // Precision for reward_per_share (1e12)
    pub const PRECISION: u128 = 1_000_000_000_000;
    
    // Maximum reasonable amount: 1 billion SOL
    pub const MAX_AMOUNT: u128 = 1_000_000_000 * 1_000_000_000;

    /// Calculate reward fee (1% of deposit)
    pub fn calculate_reward_fee(deposit_amount: u64) -> Result<u64> {
        let fee = (deposit_amount as u128)
            .checked_mul(Self::REWARD_FEE_BPS as u128)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(fee as u64)
    }

    /// Calculate platform fee (0.1% of deposit)
    pub fn calculate_platform_fee(deposit_amount: u64) -> Result<u64> {
        let fee = (deposit_amount as u128)
            .checked_mul(Self::PLATFORM_FEE_BPS as u128)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(fee as u64)
    }

    /// Credit fees to pools and update reward_per_share
    /// This is the key function that updates the accumulator
    pub fn credit_fee_to_pool(&mut self, fee_reward: u64, fee_platform: u64) -> Result<()> {
        require!(fee_reward <= Self::MAX_AMOUNT as u64, ErrorCode::FeeAmountTooLarge);
        require!(fee_platform <= Self::MAX_AMOUNT as u64, ErrorCode::FeeAmountTooLarge);
        
        // Credit platform pool
        self.platform_pool_balance = self
            .platform_pool_balance
            .checked_add(fee_platform)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        
        // Credit reward pool
        self.reward_pool_balance = self
            .reward_pool_balance
            .checked_add(fee_reward)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        
        // Update reward_per_share if there are deposits
        if self.total_deposited > 0 {
            // delta = fee_reward * PRECISION / total_deposited
            let delta = (fee_reward as u128)
                .checked_mul(Self::PRECISION)
                .ok_or(ErrorCode::CalculationOverflow)?
                .checked_div(self.total_deposited as u128)
                .ok_or(ErrorCode::CalculationOverflow)?;
            
            self.reward_per_share = self
                .reward_per_share
                .checked_add(delta)
                .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        }
        
        Ok(())
    }

    /// Calculate backer's claimable rewards using reward-per-share
    /// Formula: (deposited_amount * reward_per_share - reward_debt) / PRECISION
    pub fn calculate_claimable_rewards(&self, deposited_amount: u64, reward_debt: u128) -> Result<u64> {
        let accumulated = (deposited_amount as u128)
            .checked_mul(self.reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        let claimable = accumulated
            .checked_sub(reward_debt)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(Self::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        Ok(claimable as u64)
    }

    /// Credit reward pool (legacy method)
    pub fn credit_reward_pool(&mut self, amount: u128) -> Result<()> {
        require!(amount <= Self::MAX_AMOUNT, ErrorCode::FeeAmountTooLarge);
        self.reward_pool_balance = self
            .reward_pool_balance
            .checked_add(amount as u64)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    /// Debit reward pool (when rewards are claimed)
    pub fn debit_reward_pool(&mut self, amount: u64) -> Result<()> {
        require!(amount <= Self::MAX_AMOUNT as u64, ErrorCode::FeeAmountTooLarge);
        self.reward_pool_balance = self
            .reward_pool_balance
            .checked_sub(amount)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    /// Credit platform pool (add fees)
    pub fn credit_platform_pool(&mut self, amount: u128) -> Result<()> {
        require!(amount <= Self::MAX_AMOUNT, ErrorCode::FeeAmountTooLarge);
        self.platform_pool_balance = self
            .platform_pool_balance
            .checked_add(amount as u64)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    // Legacy methods for backward compatibility (deprecated)
    
    /// Calculate available rewards (legacy - now just returns reward_pool_balance)
    pub fn calculate_available_rewards(&self) -> u128 {
        self.reward_pool_balance as u128
    }

    /// Update APY (legacy - no-op in new model)
    pub fn update_apy(&mut self, _new_apy: u64) -> Result<()> {
        // No-op in new fee-based model
        Ok(())
    }

    /// Distribute fees (legacy - credits reward pool)
    pub fn distribute_fees(&mut self, fees: u64) -> Result<()> {
        self.credit_reward_pool(fees as u128)
    }
}
