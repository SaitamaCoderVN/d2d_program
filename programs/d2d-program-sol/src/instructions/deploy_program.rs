use crate::errors::ErrorCode;
use crate::events::ProgramDeployed;
use crate::states::{DeployRequest, DeployRequestStatus, TreasuryPool, UserDeployStats};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

#[derive(Accounts)]
#[instruction(program_hash: [u8; 32])]
pub struct DeployProgram<'info> {
    #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    #[account(
        init,
        payer = developer,
        space = 8 + DeployRequest::INIT_SPACE,
        seeds = [DeployRequest::PREFIX_SEED, program_hash.as_ref()],
        bump
    )]
    pub deploy_request: Account<'info, DeployRequest>,
    #[account(
        init_if_needed,
        payer = developer,
        space = 8 + UserDeployStats::INIT_SPACE,
        seeds = [UserDeployStats::PREFIX_SEED, developer.key().as_ref()],
        bump
    )]
    pub user_stats: Account<'info, UserDeployStats>,
    #[account(mut)]
    pub developer: Signer<'info>,
    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
    /// CHECK: Treasury wallet address - validated against treasury_pool
    #[account(
        mut,
        constraint = treasury_wallet.key() == treasury_pool.treasury_wallet @ ErrorCode::InvalidTreasuryWallet
    )]
    pub treasury_wallet: UncheckedAccount<'info>,
    /// CHECK: Ephemeral key for deployment - admin controls this
    #[account(mut)]
    pub ephemeral_key: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn deploy_program(
    ctx: Context<DeployProgram>,
    program_hash: [u8; 32],
    service_fee: u64,
    monthly_fee: u64,
    initial_months: u32,
    deployment_cost: u64,
) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;
    let user_stats = &mut ctx.accounts.user_stats;
    let current_time = Clock::get()?.unix_timestamp;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(service_fee > 0, ErrorCode::InvalidAmount);
    require!(monthly_fee > 0, ErrorCode::InvalidAmount);
    require!(initial_months > 0, ErrorCode::InvalidAmount);
    require!(deployment_cost > 0, ErrorCode::InvalidAmount);

    // Check if treasury has enough funds for deployment
    require!(
        deployment_cost <= treasury_pool.total_staked,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Initialize user stats if first time
    if user_stats.user == Pubkey::default() {
        user_stats.user = ctx.accounts.developer.key();
        user_stats.active_sessions = 0;
        user_stats.daily_deploys = 0;
        user_stats.total_deploys = 0;
        user_stats.last_reset = current_time;
        user_stats.bump = ctx.bumps.user_stats;
    }

    // Reset daily counter if new day
    if current_time - user_stats.last_reset > 86400 {
        user_stats.daily_deploys = 0;
        user_stats.last_reset = current_time;
    }

    // Calculate total payment
    let total_payment = service_fee + (monthly_fee * initial_months as u64);

    // Initialize deploy request
    deploy_request.request_id = program_hash;
    deploy_request.developer = ctx.accounts.developer.key();
    deploy_request.program_hash = program_hash;
    deploy_request.service_fee = service_fee;
    deploy_request.monthly_fee = monthly_fee;
    deploy_request.deployment_cost = deployment_cost;
    deploy_request.subscription_paid_until =
        current_time + (initial_months as i64 * 30 * 24 * 60 * 60);
    deploy_request.ephemeral_key = Some(ctx.accounts.ephemeral_key.key());
    deploy_request.deployed_program_id = None; // Will be set after actual deployment
    deploy_request.status = DeployRequestStatus::PendingDeployment;
    deploy_request.created_at = current_time;
    deploy_request.bump = ctx.bumps.deploy_request;

    // Update user stats
    user_stats.active_sessions += 1;
    user_stats.daily_deploys += 1;
    user_stats.total_deploys += 1;

    // Transfer developer payment to treasury
    let developer_payment_cpi = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.developer.to_account_info(),
            to: ctx.accounts.treasury_wallet.to_account_info(),
        },
    );
    system_program::transfer(developer_payment_cpi, total_payment)?;

    // Transfer deployment cost from treasury to ephemeral key
    let deployment_cpi = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.treasury_wallet.to_account_info(),
            to: ctx.accounts.ephemeral_key.to_account_info(),
        },
    );
    system_program::transfer(deployment_cpi, deployment_cost)?;

    // Update treasury pool
    treasury_pool.total_staked -= deployment_cost;
    treasury_pool.distribute_fees(total_payment)?;

    emit!(ProgramDeployed {
        request_id: deploy_request.request_id,
        developer: deploy_request.developer,
        program_hash: deploy_request.program_hash,
        service_fee,
        monthly_fee,
        initial_months,
        deployment_cost,
        ephemeral_key: ctx.accounts.ephemeral_key.key(),
        total_payment,
        deployed_at: current_time,
    });

    Ok(())
}
