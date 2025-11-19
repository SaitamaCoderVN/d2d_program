use crate::errors::ErrorCode;
use crate::events::ApyUpdated;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateApy<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

pub fn update_apy(ctx: Context<UpdateApy>, _new_apy: u64) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(
        ctx.accounts.admin.key() == treasury_pool.admin,
        ErrorCode::Unauthorized
    );

    // No-op in new fee-based model (APY not used)
    // Keep for backward compatibility
    treasury_pool.update_apy(0)?;

    emit!(ApyUpdated {
        old_apy: 0,
        new_apy: 0,
        updated_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
