use crate::events::TreasuryInitialized;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + TreasuryPool::INIT_SPACE,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// CHECK: Reward Pool PDA (program-owned, holds 1% fees)
    #[account(
        init,
        payer = admin,
        space = 8, // Empty account, just holds lamports
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump
    )]
    pub reward_pool: UncheckedAccount<'info>,
    
    /// CHECK: Platform Pool PDA (program-owned, holds 0.1% fees)
    #[account(
        init,
        payer = admin,
        space = 8, // Empty account, just holds lamports
        seeds = [TreasuryPool::PLATFORM_POOL_SEED],
        bump
    )]
    pub platform_pool: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    /// CHECK: Dev wallet that receives deposits for deployments
    pub dev_wallet: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn initialize(
    ctx: Context<Initialize>,
    _initial_apy: u64, // Legacy parameter, not used in new model
    dev_wallet: Pubkey,
) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;

    msg!("[INIT] Initializing Treasury Pool with new layout");
    msg!("[INIT] Account size: {} bytes", 8 + TreasuryPool::INIT_SPACE);
    msg!("[INIT] Admin: {}", ctx.accounts.admin.key());
    msg!("[INIT] Dev wallet: {}", dev_wallet);

    // Initialize fee-based system with reward-per-share
    treasury_pool.reward_per_share = 0;
    treasury_pool.total_deposited = 0;
    treasury_pool.liquid_balance = 0;
    treasury_pool.reward_pool_balance = 0;
    treasury_pool.platform_pool_balance = 0;
    treasury_pool.reward_fee_bps = TreasuryPool::REWARD_FEE_BPS;
    treasury_pool.platform_fee_bps = TreasuryPool::PLATFORM_FEE_BPS;
    
    treasury_pool.admin = ctx.accounts.admin.key();
    treasury_pool.dev_wallet = dev_wallet;
    treasury_pool.emergency_pause = false;
    
    treasury_pool.reward_pool_bump = ctx.bumps.reward_pool;
    treasury_pool.platform_pool_bump = ctx.bumps.platform_pool;
    treasury_pool.bump = ctx.bumps.treasury_pool;
    
    msg!("[INIT] Bumps - treasury: {}, reward: {}, platform: {}", 
         treasury_pool.bump, treasury_pool.reward_pool_bump, treasury_pool.platform_pool_bump);
    
    // Initialize legacy fields to 0
    treasury_pool.backer_total_staked = 0;
    treasury_pool.backer_stake_pool_bump = 0;
    treasury_pool.total_rewards_distributed = 0;
    treasury_pool.admin_pool_balance = 0;
    treasury_pool.admin_pool_bump = 0;
    treasury_pool.current_apy_bps = 0;
    treasury_pool.last_apy_update_ts = 0;
    treasury_pool.last_distribution_time = 0;
    treasury_pool.total_staked = 0;
    treasury_pool.total_fees_collected = 0;
    treasury_pool.current_apy = 0;
    treasury_pool.treasury_wallet = Pubkey::default();

    msg!("[INIT] Treasury Pool initialized successfully");
    msg!("[INIT] reward_per_share: {}", treasury_pool.reward_per_share);
    msg!("[INIT] total_deposited: {}", treasury_pool.total_deposited);
    msg!("[INIT] liquid_balance: {}", treasury_pool.liquid_balance);

    emit!(TreasuryInitialized {
        admin: treasury_pool.admin,
        treasury_wallet: dev_wallet,
        initial_apy: 0, // Not used in new model
    });

    Ok(())
}
