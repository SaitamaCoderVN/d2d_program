use crate::errors::ErrorCode;
use crate::events::ProgramsSuspended;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SuspendExpiredPrograms<'info> {
    #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

pub fn suspend_expired_programs(ctx: Context<SuspendExpiredPrograms>) -> Result<()> {
    let treasury_pool = &ctx.accounts.treasury_pool;
    let current_time = Clock::get()?.unix_timestamp;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(
        ctx.accounts.admin.key() == treasury_pool.admin,
        ErrorCode::Unauthorized
    );

    // This is a placeholder - in a real implementation, you would iterate through
    // all DeployRequest accounts and suspend expired ones
    // For now, we'll just emit an event

    emit!(ProgramsSuspended {
        suspended_count: 0, // Would be calculated in real implementation
        suspended_at: current_time,
    });

    Ok(())
}
