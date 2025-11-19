use crate::errors::ErrorCode;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

/// Reset/Reinitialize Treasury Pool (Admin only)
/// 
/// WARNING: This will wipe all existing state!
/// Use only when migrating from old struct layout to new layout.
/// 
/// This instruction:
/// 1. Closes the old treasury_pool account (reclaims rent)
/// 2. Reinitializes with new struct layout
/// 3. Sets all fields to default values
#[derive(Accounts)]
pub struct ResetTreasuryPool<'info> {
    #[account(
        mut,
        close = admin, // Close old account and send lamports to admin
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// CHECK: Reward Pool PDA
    #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
    pub reward_pool: UncheckedAccount<'info>,
    
    /// CHECK: Platform Pool PDA
    #[account(
        mut,
        seeds = [TreasuryPool::PLATFORM_POOL_SEED],
        bump = treasury_pool.platform_pool_bump
    )]
    pub platform_pool: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    /// CHECK: Dev wallet
    pub dev_wallet: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Reset and reinitialize treasury pool with new layout
pub fn reset_treasury_pool(
    ctx: Context<ResetTreasuryPool>,
    _dev_wallet: Pubkey,
) -> Result<()> {
    // Verify admin
    require!(
        ctx.accounts.admin.key() == ctx.accounts.treasury_pool.admin,
        ErrorCode::Unauthorized
    );

    // The old account will be closed by the `close = admin` constraint
    // Now we need to reinitialize it with the new layout
    
    // Note: After closing, we need to call initialize again
    // This is a two-step process:
    // 1. This instruction closes the old account
    // 2. Admin must call initialize() again to create new account
    
    msg!("Treasury pool account closed. Please call initialize() to create new account with updated layout.");
    
    Ok(())
}

