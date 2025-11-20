use crate::errors::ErrorCode;
use crate::events::RewardCredited;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;
use anchor_lang::system_program;

/// Credit fees to pools (admin/backend only)
/// 
/// This instruction is called by backend when devs pay fees.
/// Updates reward_per_share accumulator.
#[derive(Accounts)]
pub struct CreditFeeToPool<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// CHECK: Reward Pool PDA (receives reward fees)
    #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
    pub reward_pool: UncheckedAccount<'info>,
    
    /// CHECK: Platform Pool PDA (receives platform fees)
    #[account(
        mut,
        seeds = [TreasuryPool::PLATFORM_POOL_SEED],
        bump = treasury_pool.platform_pool_bump
    )]
    pub platform_pool: UncheckedAccount<'info>,
    
    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Credit fees to pools and update reward_per_share
/// 
/// Flow:
/// 1. Developer has already transferred fees to RewardPool and PlatformPool PDAs (off-chain)
/// 2. Admin calls this instruction to "record" the fees in on-chain state
/// 3. Transfer fees from admin to pools (if not already transferred)
/// 4. Call treasury_pool.credit_fee_to_pool() which updates reward_per_share
/// 
/// NOTE: This instruction does NOT transfer funds - it only updates accounting state.
/// The actual funds should already be in the pool PDAs from developer payment.
/// This updates reward_per_share accumulator for reward distribution.
pub fn credit_fee_to_pool(
    ctx: Context<CreditFeeToPool>,
    fee_reward: u64,
    fee_platform: u64,
) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(fee_reward > 0 || fee_platform > 0, ErrorCode::InvalidAmount);

    // Check admin has enough lamports
    let admin_lamports = ctx.accounts.admin.lamports();
    let total_fees = fee_reward
        .checked_add(fee_platform)
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    require!(
        admin_lamports >= total_fees,
        ErrorCode::InsufficientDeposit
    );

    // Transfer reward fee to Reward Pool PDA
    if fee_reward > 0 {
        let reward_fee_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.admin.to_account_info(),
                to: ctx.accounts.reward_pool.to_account_info(),
            },
        );
        system_program::transfer(reward_fee_cpi, fee_reward)?;
    }

    // Transfer platform fee to Platform Pool PDA
    if fee_platform > 0 {
        let platform_fee_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.admin.to_account_info(),
                to: ctx.accounts.platform_pool.to_account_info(),
            },
        );
        system_program::transfer(platform_fee_cpi, fee_platform)?;
    }

    // Credit fees to pools and update reward_per_share
    // This is the key function that updates the accumulator
    treasury_pool.credit_fee_to_pool(fee_reward, fee_platform)?;

    emit!(RewardCredited {
        fee_reward,
        fee_platform,
        reward_per_share: treasury_pool.reward_per_share,
        total_deposited: treasury_pool.total_deposited,
        credited_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

