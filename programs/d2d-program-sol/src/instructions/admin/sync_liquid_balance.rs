use crate::errors::ErrorCode;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

/// Sync liquid_balance with actual account balance
/// Admin-only instruction to fix liquid_balance when it's out of sync
/// 
/// This is useful when:
/// - Account balance is higher than liquid_balance (e.g., from direct transfers)
/// - liquid_balance needs to be updated to match actual account balance
#[derive(Accounts)]
pub struct SyncLiquidBalance<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    /// CHECK: Treasury Pool PDA (to get actual account balance)
    #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pda: UncheckedAccount<'info>,

    #[account(
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
}

/// Sync liquid_balance with actual account balance
/// 
/// This instruction:
/// 1. Gets the actual account balance (lamports) from treasury_pda
/// 2. Calculates rent exemption
/// 3. Updates liquid_balance to match (account_balance - rent_exemption)
/// 
/// This ensures liquid_balance reflects the actual available SOL in the account
pub fn sync_liquid_balance(ctx: Context<SyncLiquidBalance>) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

    // Get actual account balance
    let actual_account_balance = treasury_pda_info.lamports();
    
    // Calculate rent exemption
    let account_data_size = treasury_pda_info.data_len();
    let rent_exemption = Rent::get()?.minimum_balance(account_data_size);
    
    // Available balance = actual balance - rent exemption
    let available_balance = actual_account_balance
        .checked_sub(rent_exemption)
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    // Update liquid_balance to match available balance
    treasury_pool.liquid_balance = available_balance;

    msg!("[SYNC] Synced liquid_balance with account balance");
    msg!("[SYNC] Account balance: {} lamports", actual_account_balance);
    msg!("[SYNC] Rent exemption: {} lamports", rent_exemption);
    msg!("[SYNC] Available balance: {} lamports", available_balance);
    msg!("[SYNC] Updated liquid_balance: {} lamports", treasury_pool.liquid_balance);

    Ok(())
}

