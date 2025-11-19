use crate::errors::ErrorCode;
use crate::events::ProgramClosed;
use crate::states::{DeployRequest, DeployRequestStatus, TreasuryPool};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

/// Close a deployed program and refund recovered lamports to pool
/// This is called after a program is closed on-chain
#[derive(Accounts)]
#[instruction(request_id: [u8; 32])]
pub struct CloseProgramAndRefund<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    #[account(
        mut,
        seeds = [DeployRequest::PREFIX_SEED, request_id.as_ref()],
        bump = deploy_request.bump,
        constraint = deploy_request.status == DeployRequestStatus::Active @ ErrorCode::InvalidDeploymentStatus
    )]
    pub deploy_request: Account<'info, DeployRequest>,

    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,

    /// CHECK: Account that will send recovered lamports (could be program account or ephemeral key)
    #[account(mut)]
    pub refund_source: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn close_program_and_refund(
    ctx: Context<CloseProgramAndRefund>,
    _request_id: [u8; 32],
    recovered_lamports: u64,
) -> Result<()> {
    // Get account info before mutable borrow
    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;
    let current_time = Clock::get()?.unix_timestamp;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(recovered_lamports > 0, ErrorCode::InvalidAmount);

    // Transfer recovered lamports directly to Treasury Pool PDA
    let cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.refund_source.to_account_info(),
            to: treasury_pool_info.clone(),
        },
    );
    system_program::transfer(cpi_context, recovered_lamports)?;

    // Update treasury pool balance
    treasury_pool.total_staked += recovered_lamports;

    // Mark deploy request as closed
    deploy_request.status = DeployRequestStatus::Closed;

    emit!(ProgramClosed {
        request_id: deploy_request.request_id,
        program_id: deploy_request.deployed_program_id.unwrap_or_default(),
        developer: deploy_request.developer,
        recovered_lamports,
        closed_at: current_time,
    });

    Ok(())
}

