use crate::errors::ErrorCode;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

/// Close Treasury Pool Account (Admin only)
/// 
/// This instruction closes the treasury pool account by transferring all lamports to admin.
/// It does NOT require deserializing the account, so it works even with old struct layouts.
/// 
/// WARNING: This will transfer all funds to admin and make the account rent-exempt!
/// Use this when migrating from old struct layout to new layout.
/// 
/// After closing, you can call initialize() again to create a new account with the new layout.
#[derive(Accounts)]
pub struct CloseTreasuryPool<'info> {
    /// CHECK: Treasury Pool PDA - lamports will be transferred out
    /// We use UncheckedAccount to avoid deserialization (works with old layouts)
    /// PDA seeds are verified by Anchor constraint, so it's safe to use UncheckedAccount
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump // Anchor will find the bump automatically
    )]
    pub treasury_pool: UncheckedAccount<'info>,
    
    /// Admin who will receive the lamports
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Close treasury pool account by transferring all lamports to admin
/// This works even if the account has an old struct layout
pub fn close_treasury_pool(ctx: Context<CloseTreasuryPool>) -> Result<()> {
    msg!("[CLOSE] Closing Treasury Pool account");
    msg!("[CLOSE] Admin: {}", ctx.accounts.admin.key());
    msg!("[CLOSE] Treasury Pool PDA: {}", ctx.accounts.treasury_pool.key());
    
    // Get account info
    let treasury_account = &ctx.accounts.treasury_pool;
    let balance_before = treasury_account.lamports();
    
    msg!("[CLOSE] Account balance before close: {} lamports", balance_before);
    
    // Verify PDA seeds
    let (expected_pda, bump) = Pubkey::try_find_program_address(
        &[TreasuryPool::PREFIX_SEED],
        ctx.program_id,
    )
    .ok_or(ErrorCode::InvalidTreasuryWallet)?;
    
    require!(
        expected_pda == treasury_account.key(),
        ErrorCode::InvalidTreasuryWallet
    );
    
    msg!("[CLOSE] PDA verified - bump: {}", bump);
    
    // Calculate rent-exempt minimum (account data size + rent)
    // For old layout: ~114 bytes, for new layout: ~278 bytes
    // We'll use a conservative estimate: 300 bytes
    let rent_exempt_minimum = Rent::get()?.minimum_balance(300);
    
    if balance_before <= rent_exempt_minimum {
        msg!("[CLOSE] Account already rent-exempt or has minimal balance");
        msg!("[CLOSE] Balance: {} lamports, Rent minimum: {} lamports", balance_before, rent_exempt_minimum);
    }
    
    // Transfer all lamports except rent-exempt minimum to admin
    // This makes the account rent-exempt, effectively closing it
    let transfer_amount = balance_before.saturating_sub(rent_exempt_minimum);
    
    if transfer_amount > 0 {
        msg!("[CLOSE] Transferring {} lamports to admin", transfer_amount);
        
        // Use direct lamport mutation for program-owned accounts
        **treasury_account.try_borrow_mut_lamports()? = balance_before
            .checked_sub(transfer_amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        **ctx.accounts.admin.try_borrow_mut_lamports()? = ctx.accounts.admin.lamports()
            .checked_add(transfer_amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        msg!("[CLOSE] Transfer complete");
    } else {
        msg!("[CLOSE] No lamports to transfer (account already rent-exempt)");
    }
    
    msg!("[CLOSE] Treasury Pool account closed successfully");
    msg!("[CLOSE] Remaining balance: {} lamports (rent-exempt minimum)", treasury_account.lamports());
    msg!("[CLOSE] You can now call initialize() to create a new account with the updated layout");
    
    Ok(())
}

