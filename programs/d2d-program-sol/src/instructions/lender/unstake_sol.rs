use crate::errors::ErrorCode;
use crate::events::SolUnstaked;
use crate::states::{BackerDeposit, TreasuryPool};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

/// Unstake SOL (withdraw deposit)
/// 
/// Reward-per-share model:
/// - If liquid_balance >= amount: withdraw immediately
/// - Else: create withdraw_request (to be implemented)
#[derive(Accounts)]
pub struct UnstakeSol<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// CHECK: Treasury Pool PDA (holds deposits)
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pda: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [BackerDeposit::PREFIX_SEED, lender.key().as_ref()],
        bump = lender_stake.bump
    )]
    pub lender_stake: Account<'info, BackerDeposit>,
    
    #[account(mut)]
    pub lender: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Unstake SOL (withdraw principal)
/// 
/// If liquid_balance >= amount: withdraw immediately
/// Else: return error (withdraw_request to be implemented separately)
pub fn unstake_sol(ctx: Context<UnstakeSol>, amount: u64) -> Result<()> {
    // Get account info and bump before mutable borrows
    let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();
    let treasury_bump = ctx.accounts.treasury_pool.bump;
    
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let lender_stake = &mut ctx.accounts.lender_stake;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(lender_stake.is_active, ErrorCode::InactiveStake);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        amount <= lender_stake.deposited_amount,
        ErrorCode::InsufficientStake
    );

    // Check if liquid balance is sufficient
    if treasury_pool.liquid_balance < amount {
        // Insufficient liquid balance - would need withdraw_request
        // For now, return error
        return Err(ErrorCode::InsufficientLiquidBalance.into());
    }

    // Check Treasury PDA has enough lamports
    let treasury_lamports = treasury_pda_info.lamports();
    require!(
        treasury_lamports >= amount,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Update backer deposit
    lender_stake.deposited_amount = lender_stake
        .deposited_amount
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // If fully withdrawn, deactivate
    if lender_stake.deposited_amount == 0 {
        lender_stake.is_active = false;
        lender_stake.reward_debt = 0;
    } else {
        // Update reward_debt for remaining deposit
        lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;
    }

    // Update treasury pool state
    treasury_pool.total_deposited = treasury_pool
        .total_deposited
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    treasury_pool.liquid_balance = treasury_pool
        .liquid_balance
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // Transfer principal from Treasury PDA -> lender
    // Use PDA seeds for signing (program-owned account)
    let treasury_seeds = &[
        TreasuryPool::PREFIX_SEED,
        &[treasury_bump],
    ];
    let signer_seeds = &[&treasury_seeds[..]];
    
    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.treasury_pda.to_account_info(),
            to: ctx.accounts.lender.to_account_info(),
        },
        signer_seeds,
    );
    system_program::transfer(cpi_context, amount)?;

    emit!(SolUnstaked {
        lender: lender_stake.backer,
        amount, // Only principal, no rewards
        remaining_staked: lender_stake.deposited_amount,
    });

    Ok(())
}
