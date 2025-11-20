use crate::errors::ErrorCode;
use crate::events::RewardsClaimed;
use crate::states::{LenderStake, TreasuryPool};
use anchor_lang::prelude::*;

/// Claim accumulated rewards (reward-per-share model)
/// 
/// Flow:
/// 1. Calculate claimable = (deposited_amount * reward_per_share - reward_debt) / PRECISION
/// 2. Verify reward_pool has enough lamports
/// 3. Transfer from reward_pool PDA -> backer (via lamport mutation)
/// 4. Update reward_debt and claimed_total
#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// CHECK: Reward Pool PDA (holds reward fees)
    #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
    pub reward_pool: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [LenderStake::PREFIX_SEED, lender.key().as_ref()],
        bump = lender_stake.bump
    )]
    pub lender_stake: Account<'info, LenderStake>,
    
    #[account(mut)]
    pub lender: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Claim rewards (reward-per-share model)
pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
    msg!("[CLAIM] Starting claim_rewards instruction");
    msg!("[CLAIM] Lender: {}", ctx.accounts.lender.key());
    
    // Get account info before mutable borrows
    let reward_pool_info = ctx.accounts.reward_pool.to_account_info();
    
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let lender_stake = &mut ctx.accounts.lender_stake;

    msg!("[CLAIM] Treasury Pool loaded - reward_per_share: {}, reward_pool_balance: {}", 
         treasury_pool.reward_per_share, treasury_pool.reward_pool_balance);
    msg!("[CLAIM] Lender Stake - deposited_amount: {}, reward_debt: {}", 
         lender_stake.deposited_amount, lender_stake.reward_debt);

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(lender_stake.is_active, ErrorCode::InactiveStake);

    // Calculate claimable rewards using reward-per-share
    let claimable_rewards = lender_stake.calculate_claimable_rewards(treasury_pool.reward_per_share)?;
    msg!("[CLAIM] Calculated claimable rewards: {} lamports", claimable_rewards);
    require!(claimable_rewards > 0, ErrorCode::NoRewardsToClaim);

    // Verify reward pool has enough balance
    require!(
        treasury_pool.reward_pool_balance >= claimable_rewards,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Check Reward Pool PDA has enough lamports
    let reward_pool_lamports = reward_pool_info.lamports();
    require!(
        reward_pool_lamports >= claimable_rewards,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Update lender stake
    lender_stake.claimed_total = lender_stake
        .claimed_total
        .checked_add(claimable_rewards)
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    // Update reward_debt to current accumulated value
    lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;

    // Debit reward pool balance
    treasury_pool.debit_reward_pool(claimable_rewards)?;

    // Transfer rewards from Reward Pool PDA -> lender
    // CRITICAL: Use lamport mutation for program-owned accounts (not CPI System transfer)
    // Reward Pool PDA may have data, so we cannot use System Program transfer
    {
        let lender_info = ctx.accounts.lender.to_account_info();
        let mut reward_pool_lamports = reward_pool_info.try_borrow_mut_lamports()?;
        let mut lender_lamports = lender_info.try_borrow_mut_lamports()?;

        **reward_pool_lamports = (**reward_pool_lamports)
            .checked_sub(claimable_rewards)
            .ok_or(ErrorCode::CalculationOverflow)?;
        **lender_lamports = (**lender_lamports)
            .checked_add(claimable_rewards)
            .ok_or(ErrorCode::CalculationOverflow)?;
    }

    emit!(RewardsClaimed {
        lender: lender_stake.backer,
        amount: claimable_rewards,
        total_claimed: lender_stake.claimed_total,
    });
    
    // Emit detailed claim event
    emit!(crate::events::Claimed {
        backer: lender_stake.backer,
        amount: claimable_rewards,
        claimed_total: lender_stake.claimed_total,
        reward_per_share: treasury_pool.reward_per_share,
        claimed_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
