use crate::events::TreasuryInitialized;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

/// Reinitialize Treasury Pool (Admin only)
/// 
/// This instruction reinitializes an existing treasury pool account with new struct layout.
/// It works even if the account has old layout or is rent-exempt.
/// 
/// This is used after closing the old account to migrate to new layout.
#[derive(Accounts)]
pub struct ReinitializeTreasuryPool<'info> {
    /// CHECK: Treasury Pool PDA - will be reinitialized
    /// We use UncheckedAccount to avoid deserialization, then manually resize and initialize
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
    pub treasury_pool: UncheckedAccount<'info>,
    
    /// CHECK: Reward Pool PDA
    #[account(
        init_if_needed,
        payer = admin,
        space = 8,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump
    )]
    pub reward_pool: UncheckedAccount<'info>,
    
    /// CHECK: Platform Pool PDA
    #[account(
        init_if_needed,
        payer = admin,
        space = 8,
        seeds = [TreasuryPool::PLATFORM_POOL_SEED],
        bump
    )]
    pub platform_pool: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    /// CHECK: Dev wallet
    pub dev_wallet: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Reinitialize treasury pool with new layout
pub fn reinitialize_treasury_pool(
    ctx: Context<ReinitializeTreasuryPool>,
    _initial_apy: u64, // Legacy parameter, not used in new model
    dev_wallet: Pubkey,
) -> Result<()> {
    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let required_space = 8 + TreasuryPool::INIT_SPACE;
    
    // Resize account if needed
    let current_space = treasury_pool_info.data_len();
    if current_space < required_space {
        msg!("[REINIT] Resizing account from {} to {} bytes", current_space, required_space);
        treasury_pool_info.realloc(required_space, false)?;
    }
    
    // Zero out the account data to ensure clean initialization
    let mut data = treasury_pool_info.try_borrow_mut_data()?;
    data[..].fill(0);
    
    // Create new TreasuryPool struct with all fields initialized
    let mut treasury_pool = TreasuryPool {
        reward_per_share: 0,
        total_deposited: 0,
        liquid_balance: 0,
        reward_pool_balance: 0,
        platform_pool_balance: 0,
        reward_fee_bps: TreasuryPool::REWARD_FEE_BPS,
        platform_fee_bps: TreasuryPool::PLATFORM_FEE_BPS,
        admin: ctx.accounts.admin.key(),
        dev_wallet: dev_wallet,
        emergency_pause: false,
        reward_pool_bump: ctx.bumps.reward_pool,
        platform_pool_bump: ctx.bumps.platform_pool,
        bump: ctx.bumps.treasury_pool,
        // Legacy fields
        backer_total_staked: 0,
        backer_stake_pool_bump: 0,
        total_rewards_distributed: 0,
        admin_pool_balance: 0,
        admin_pool_bump: 0,
        current_apy_bps: 0,
        last_apy_update_ts: 0,
        last_distribution_time: 0,
        total_staked: 0,
        total_fees_collected: 0,
        current_apy: 0,
        treasury_wallet: Pubkey::default(),
    };

    msg!("[REINIT] Reinitializing Treasury Pool with new layout");
    msg!("[REINIT] Account size: {} bytes", required_space);
    msg!("[REINIT] Admin: {}", ctx.accounts.admin.key());
    msg!("[REINIT] Dev wallet: {}", dev_wallet);
    msg!("[REINIT] Bumps - treasury: {}, reward: {}, platform: {}", 
         treasury_pool.bump, treasury_pool.reward_pool_bump, treasury_pool.platform_pool_bump);
    
    // Serialize to account
    treasury_pool.try_serialize(&mut &mut data[..])?;

    msg!("[REINIT] Treasury Pool reinitialized successfully");
    msg!("[REINIT] reward_per_share: {}", treasury_pool.reward_per_share);
    msg!("[REINIT] total_deposited: {}", treasury_pool.total_deposited);
    msg!("[REINIT] liquid_balance: {}", treasury_pool.liquid_balance);

    // Emit event
    emit!(TreasuryInitialized {
        admin: ctx.accounts.admin.key(),
        treasury_wallet: dev_wallet,
        initial_apy: 0, // Not used in new model
    });

    Ok(())
}

