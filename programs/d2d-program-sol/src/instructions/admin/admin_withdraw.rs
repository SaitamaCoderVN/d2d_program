use crate::errors::ErrorCode;
use crate::events::AdminWithdrew;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

/// Admin withdraw funds from Admin Pool
/// 
/// Safety: Only admin can withdraw, with event logging for audit
#[derive(Accounts)]
pub struct AdminWithdraw<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// CHECK: Admin Pool PDA (program-owned, holds platform funds)
    #[account(
        mut,
        seeds = [TreasuryPool::ADMIN_POOL_SEED],
        bump = treasury_pool.admin_pool_bump
    )]
    pub admin_pool: UncheckedAccount<'info>,
    
    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
    
    /// CHECK: Destination wallet for withdrawal
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Admin withdraw from Admin Pool
/// 
/// Flow:
/// 1. Verify admin authorization
/// 2. Check Admin Pool has enough lamports
/// 3. Transfer from Admin Pool PDA -> destination (via lamport mutation or CPI)
/// 4. Update admin_pool_balance in state
pub fn admin_withdraw(
    ctx: Context<AdminWithdraw>,
    amount: u64,
    reason: String,
) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let admin_pool_info = ctx.accounts.admin_pool.to_account_info();
    let destination_info = ctx.accounts.destination.to_account_info();

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        treasury_pool.admin_pool_balance >= amount as u128,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Check Admin Pool PDA has enough lamports
    require!(
        admin_pool_info.lamports() >= amount,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Transfer from Admin Pool PDA -> destination
    // Use lamport mutation for program-owned account
    {
        let mut admin_pool_lamports = admin_pool_info.try_borrow_mut_lamports()?;
        let mut destination_lamports = destination_info.try_borrow_mut_lamports()?;

        **admin_pool_lamports = (**admin_pool_lamports)
            .checked_sub(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        **destination_lamports = (**destination_lamports)
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
    }

    // Update Admin Pool balance in state
    treasury_pool.admin_pool_balance = treasury_pool
        .admin_pool_balance
        .checked_sub(amount as u128)
        .ok_or(ErrorCode::CalculationOverflow)?;

    emit!(AdminWithdrew {
        admin: ctx.accounts.admin.key(),
        amount,
        destination: destination_info.key(),
        reason,
        withdrawn_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

