use crate::errors::ErrorCode;
use crate::events::EmergencyPauseToggled;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

pub fn emergency_pause(ctx: Context<EmergencyPause>, pause: bool) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;

    require!(
        ctx.accounts.admin.key() == treasury_pool.admin,
        ErrorCode::Unauthorized
    );

    treasury_pool.emergency_pause = pause;

    emit!(EmergencyPauseToggled {
        paused: pause,
        toggled_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
