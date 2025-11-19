use crate::errors::ErrorCode;
use crate::events::SubscriptionPaid;
use crate::states::{DeployRequest, DeployRequestStatus, TreasuryPool};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

#[derive(Accounts)]
pub struct PaySubscription<'info> {
    #[account(
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
    #[account(mut)]
    pub developer: Signer<'info>,
    /// CHECK: Treasury wallet address - validated against treasury_pool
    #[account(
        mut,
        constraint = treasury_wallet.key() == treasury_pool.treasury_wallet @ ErrorCode::InvalidTreasuryWallet
    )]
    pub treasury_wallet: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn pay_subscription(
    ctx: Context<PaySubscription>,
    request_id: [u8; 32],
    months: u32,
) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(
        deploy_request.request_id == request_id,
        ErrorCode::InvalidRequestId
    );
    require!(
        deploy_request.developer == ctx.accounts.developer.key(),
        ErrorCode::Unauthorized
    );
    require!(months > 0, ErrorCode::InvalidAmount);
    require!(
        deploy_request.status == DeployRequestStatus::Active
            || deploy_request.status == DeployRequestStatus::SubscriptionExpired,
        ErrorCode::InvalidRequestStatus
    );

    // Calculate payment amount
    let payment_amount = deploy_request.monthly_fee * months as u64;

    // Extend subscription
    deploy_request.extend_subscription(months);

    // Update status to active
    deploy_request.status = DeployRequestStatus::Active;

    // Update treasury pool
    treasury_pool.distribute_fees(payment_amount)?;

    // Transfer payment to treasury
    let cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.developer.to_account_info(),
            to: ctx.accounts.treasury_wallet.to_account_info(),
        },
    );
    system_program::transfer(cpi_context, payment_amount)?;

    emit!(SubscriptionPaid {
        request_id: deploy_request.request_id,
        developer: deploy_request.developer,
        months,
        payment_amount,
        subscription_valid_until: deploy_request.subscription_paid_until,
    });

    Ok(())
}
