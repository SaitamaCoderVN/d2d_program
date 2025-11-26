use crate::errors::ErrorCode;
use crate::events::{DeploymentConfirmed, DeploymentFailed};
use crate::states::{DeployRequest, DeployRequestStatus, TreasuryPool};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

#[derive(Accounts)]
pub struct ConfirmDeployment<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    #[account(
        mut,
        seeds = [DeployRequest::PREFIX_SEED, deploy_request.program_hash.as_ref()],
        bump = deploy_request.bump
    )]
    pub deploy_request: Account<'info, DeployRequest>,
    
    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
    
    /// CHECK: Ephemeral key that received deployment funds (must be signer for transfer)
    #[account(mut)]
    pub ephemeral_key: Signer<'info>,
    
    /// CHECK: Developer wallet for refund if deployment fails
    #[account(mut)]
    pub developer_wallet: UncheckedAccount<'info>,
    
    /// CHECK: Treasury Pool PDA (for recovered funds transfer)
    /// Note: Recovered funds go back to TreasuryPool, not PlatformPool
    /// PlatformPool only receives 0.1% developer fees
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pda: UncheckedAccount<'info>,
    
    /// CHECK: Reward Pool PDA (for refunds on failure)
    #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
    pub reward_pool: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn confirm_deployment_success(
    ctx: Context<ConfirmDeployment>,
    request_id: [u8; 32],
    deployed_program_id: Pubkey,
    recovered_funds: u64,
) -> Result<()> {
    // Get account infos before mutable borrows
    let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();
    let ephemeral_key_info = ctx.accounts.ephemeral_key.to_account_info();
    
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(
        deploy_request.request_id == request_id,
        ErrorCode::InvalidRequestId
    );
    require!(
        deploy_request.status == DeployRequestStatus::PendingDeployment,
        ErrorCode::InvalidRequestStatus
    );

    // Validate recovered funds don't exceed deployment cost
    require!(
        recovered_funds <= deploy_request.deployment_cost,
        ErrorCode::InvalidRecoveredFunds
    );

    // Verify ephemeral_key matches the one in deploy_request
    if let Some(expected_ephemeral) = deploy_request.ephemeral_key {
        require!(
            ephemeral_key_info.key() == expected_ephemeral,
            ErrorCode::InvalidEphemeralKey
        );
    }

    // Update deploy request
    deploy_request.status = DeployRequestStatus::Active;
    deploy_request.deployed_program_id = Some(deployed_program_id);
    // borrowed_amount is already set in fund_temporary_wallet

    // If there are recovered funds, transfer them back to Platform Pool
    // Note: Recovered funds go to Platform Pool (not Reward Pool) as they're operational funds
    // Note: Only recover what's actually available in ephemeral key (may have been partially drained)
    let ephemeral_balance = ephemeral_key_info.lamports();
    let actual_recovered = if recovered_funds > 0 && ephemeral_balance > 0 {
        // Recover the minimum of: requested amount and actual balance
        // This handles cases where ephemeral key was partially drained before confirmation
        recovered_funds.min(ephemeral_balance)
    } else {
        0
    };

    if actual_recovered > 0 {
        // Transfer recovered funds back to Treasury Pool PDA via CPI System Program transfer
        // CRITICAL: Recovered funds go to TreasuryPool (liquid_balance), NOT PlatformPool
        // PlatformPool only receives 0.1% developer fees, not operational funds
        // Note: ephemeral_key must be a signer for this transfer
        let treasury_seeds = &[
            TreasuryPool::PREFIX_SEED,
            &[treasury_pool.bump],
        ];
        let signer_seeds = &[&treasury_seeds[..]];
        
        // Use CPI System Program transfer from ephemeral_key to treasury_pda
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ephemeral_key_info,
                to: treasury_pda_info,
            },
        );
        system_program::transfer(cpi_context, actual_recovered)?;

        // Update liquid_balance (recovered funds are available for withdrawals)
        // This is the correct place for recovered deployment funds
        treasury_pool.liquid_balance = treasury_pool
            .liquid_balance
            .checked_add(actual_recovered)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        // NOTE: Do NOT update platform_pool_balance
        // PlatformPool only receives 0.1% developer fees, not recovered deployment funds
    }

    emit!(DeploymentConfirmed {
        request_id: deploy_request.request_id,
        developer: deploy_request.developer,
        deployed_program_id,
        deployment_cost: deploy_request.deployment_cost,
        recovered_funds: actual_recovered, // Emit actual recovered amount, not requested
        confirmed_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

pub fn confirm_deployment_failure(
    ctx: Context<ConfirmDeployment>,
    request_id: [u8; 32],
    failure_reason: String,
) -> Result<()> {
    let reward_pool_info = ctx.accounts.reward_pool.to_account_info();
    let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();
    let ephemeral_key_info = ctx.accounts.ephemeral_key.to_account_info();
    
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(
        deploy_request.request_id == request_id,
        ErrorCode::InvalidRequestId
    );
    require!(
        deploy_request.status == DeployRequestStatus::PendingDeployment,
        ErrorCode::InvalidRequestStatus
    );

    // Calculate refund amount
    let total_payment = deploy_request.service_fee
        .checked_add(deploy_request.monthly_fee)
        .ok_or(ErrorCode::CalculationOverflow)?;
    let refund_amount = total_payment; // Full refund for failed deployment

    // Validate refund amount is reasonable
    require!(
        refund_amount <= TreasuryPool::MAX_FEE_AMOUNT as u64,
        ErrorCode::FeeAmountTooLarge
    );

    // Update deploy request
    deploy_request.status = DeployRequestStatus::Failed;

    // Check Reward Pool has enough lamports for refund
    let reward_pool_lamports = reward_pool_info.lamports();
    require!(
        reward_pool_lamports >= refund_amount,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Refund developer payment from Reward Pool PDA via direct lamport manipulation
    {
        let developer_wallet_info = ctx.accounts.developer_wallet.to_account_info();
        let mut reward_pool_lamports_mut = reward_pool_info.try_borrow_mut_lamports()?;
        let mut developer_lamports = developer_wallet_info.try_borrow_mut_lamports()?;

        **reward_pool_lamports_mut = (**reward_pool_lamports_mut)
            .checked_sub(refund_amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        **developer_lamports = (**developer_lamports)
            .checked_add(refund_amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
    }
 
    // Return deployment cost to liquid_balance (where it came from)
    // Recovered funds increase liquid_balance for withdrawals
    // CRITICAL: Recovered funds go to TreasuryPool, NOT PlatformPool
    let remaining_funds = ephemeral_key_info.lamports();
    if remaining_funds > 0 {
        {
            let mut treasury_lamports = treasury_pda_info.try_borrow_mut_lamports()?;
            let mut ephemeral_lamports = ephemeral_key_info.try_borrow_mut_lamports()?;
            
            **treasury_lamports = (**treasury_lamports)
                .checked_add(remaining_funds)
                .ok_or(ErrorCode::CalculationOverflow)?;
            **ephemeral_lamports = 0; // Empty ephemeral key
        }
        
        // Update liquid_balance (recovered funds available for withdrawals)
        // This is the correct place for recovered deployment funds
        treasury_pool.liquid_balance = treasury_pool
            .liquid_balance
            .checked_add(remaining_funds)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        // NOTE: Do NOT update platform_pool_balance
        // PlatformPool only receives 0.1% developer fees, not recovered deployment funds
    }

    // IMPORTANT: Refund fees collected (decrease reward_pool_balance)
    treasury_pool.debit_reward_pool(refund_amount)?;

    emit!(DeploymentFailed {
        request_id: deploy_request.request_id,
        developer: deploy_request.developer,
        failure_reason,
        refund_amount,
        deployment_cost_returned: deploy_request.deployment_cost,
        failed_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
