use crate::errors::ErrorCode;
use crate::events::AdminWithdrew;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;
use anchor_lang::system_program;

/// Authorized admin for withdrawing excess rewards from Reward Pool
/// This admin can only withdraw the excess (surplus) after all backers' claimable rewards are accounted for
const AUTHORIZED_REWARD_ADMIN: Pubkey = anchor_lang::solana_program::pubkey!("A1dVA8adW1XXgcVmLCtbrvbVEVA1n3Q7kNPaTZVonjpq");

/// Admin withdraw funds from Reward Pool
/// 
/// Safety: Only the authorized reward admin can withdraw excess rewards
/// This allows withdrawing surplus rewards that exceed total claimable rewards
/// 
/// IMPORTANT: This should only be used to withdraw excess/surplus rewards.
/// The backend must calculate total claimable rewards for all backers first,
/// and only allow withdrawal of: reward_pool_balance - total_claimable_rewards
#[derive(Accounts)]
pub struct AdminWithdrawRewardPool<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// CHECK: Reward Pool PDA (program-owned, holds reward funds)
    #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
    pub reward_pool: UncheckedAccount<'info>,
    
    /// CHECK: Only the authorized reward admin can withdraw
    #[account(
        constraint = admin.key() == AUTHORIZED_REWARD_ADMIN @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
    
    /// CHECK: Destination wallet for withdrawal
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Admin withdraw excess funds from Reward Pool
/// 
/// Flow:
/// 1. Verify admin is the authorized reward admin
/// 2. Check Reward Pool has enough lamports
/// 3. Transfer from Reward Pool PDA -> destination (via CPI)
/// 4. Update reward_pool_balance in state
/// 
/// IMPORTANT: This instruction only allows withdrawing EXCESS rewards.
/// The backend must calculate total claimable rewards for all backers first,
/// and only allow withdrawal of: reward_pool_balance - total_claimable_rewards
/// 
/// This ensures that backers' claimable rewards are always protected.
pub fn admin_withdraw_reward_pool(
    ctx: Context<AdminWithdrawRewardPool>,
    amount: u64,
    reason: String,
) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let reward_pool_info = ctx.accounts.reward_pool.to_account_info();
    let destination_info = ctx.accounts.destination.to_account_info();

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(amount > 0, ErrorCode::InvalidAmount);
    
    // Verify admin is the authorized reward admin
    require!(
        ctx.accounts.admin.key() == AUTHORIZED_REWARD_ADMIN,
        ErrorCode::Unauthorized
    );
    
    // Check tracked balance in struct
    require!(
        treasury_pool.reward_pool_balance >= amount,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Check actual Reward Pool PDA has enough lamports
    let actual_balance = reward_pool_info.lamports();
    require!(
        actual_balance >= amount,
        ErrorCode::InsufficientTreasuryFunds
    );
    
    msg!("[ADMIN_WITHDRAW_REWARD] Authorized admin {} withdrawing {} lamports", 
         ctx.accounts.admin.key(), amount);
    msg!("[ADMIN_WITHDRAW_REWARD] Reward Pool balance before: {} lamports", 
         treasury_pool.reward_pool_balance);

    // Transfer from Reward Pool PDA -> destination via CPI
    let cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: reward_pool_info.clone(),
            to: destination_info.clone(),
        },
    );
    system_program::transfer(cpi_context, amount)?;

    // Update tracked balance in struct
    treasury_pool.reward_pool_balance = treasury_pool
        .reward_pool_balance
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    msg!("[ADMIN_WITHDRAW_REWARD] Admin {} withdrew {} lamports from Reward Pool", 
         ctx.accounts.admin.key(), amount);
    msg!("[ADMIN_WITHDRAW_REWARD] Reason: {}", reason);
    msg!("[ADMIN_WITHDRAW_REWARD] Remaining balance: {} lamports", 
         treasury_pool.reward_pool_balance);

    emit!(AdminWithdrew {
        admin: ctx.accounts.admin.key(),
        amount,
        destination: destination_info.key(),
        reason,
        withdrawn_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

