use crate::errors::ErrorCode;
use crate::events::DeploymentFundsRequested;
use crate::states::{DeployRequest, DeployRequestStatus, TreasuryPool, UserDeployStats};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

/// Request deployment funds from treasury pool
/// This instruction:
/// 1. Developer pays service fee + subscription
/// 2. Validates treasury has sufficient funds for deployment
/// 3. Creates a deploy_request with status PendingDeployment
/// 4. Backend will then call fund_temporary_wallet to get deployment funds
#[derive(Accounts)]
#[instruction(program_hash: [u8; 32])]
pub struct RequestDeploymentFunds<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    #[account(
        init_if_needed,
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
    
    /// CHECK: Treasury wallet address - validated against treasury_pool (not used for transfers)
    #[account(
        constraint = treasury_wallet.key() == treasury_pool.treasury_wallet @ ErrorCode::InvalidTreasuryWallet
    )]
    pub treasury_wallet: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn request_deployment_funds(
    ctx: Context<RequestDeploymentFunds>,
    program_hash: [u8; 32],
    service_fee: u64,
    monthly_fee: u64,
    initial_months: u32,
    deployment_cost: u64,
) -> Result<()> {
    // Get account infos before mutable borrows to avoid borrow checker issues
    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let _treasury_pool_bump = ctx.accounts.treasury_pool.bump;
    
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;
    let user_stats = &mut ctx.accounts.user_stats;
    let current_time = Clock::get()?.unix_timestamp;
 
    let is_new_deploy_request =
        deploy_request.request_id == [0u8; 32] && deploy_request.developer == Pubkey::default();

    // Assign bump provided by Anchor (available for init / init_if_needed)
    deploy_request.bump = ctx.bumps.deploy_request;

    // Validation
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

    // Calculate total payment (service fee + subscription)
    let total_payment = service_fee + (monthly_fee * initial_months as u64);

    // Initialize deploy request with PendingDeployment status
    if is_new_deploy_request {
        deploy_request.request_id = program_hash;
        deploy_request.developer = ctx.accounts.developer.key();
        deploy_request.program_hash = program_hash;
        deploy_request.created_at = current_time;
    } else {
        // Ensure this PDA corresponds to the provided hash/developer
        require!(
            deploy_request.program_hash == program_hash
                && deploy_request.developer == ctx.accounts.developer.key(),
            ErrorCode::InvalidRequestId
        );
    }

    deploy_request.service_fee = service_fee;
    deploy_request.monthly_fee = monthly_fee;
    deploy_request.deployment_cost = deployment_cost;
    deploy_request.subscription_paid_until =
        current_time + (initial_months as i64 * 30 * 24 * 60 * 60);
    deploy_request.ephemeral_key = None; // Will be set when backend funds temporary wallet
    deploy_request.deployed_program_id = None; // Will be set after backend deploys
    deploy_request.status = DeployRequestStatus::PendingDeployment;

    // Update user stats
    user_stats.active_sessions += 1;
    user_stats.daily_deploys += 1;
    user_stats.total_deploys += 1;

    // Transfer developer payment (service fee + subscription) directly to Treasury Pool PDA
    let developer_payment_cpi = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.developer.to_account_info(),
            to: treasury_pool_info.clone(),
        },
    );
    system_program::transfer(developer_payment_cpi, total_payment)?;

    // Note: Deployment cost will be transferred later via fund_temporary_wallet instruction
    // This separates developer payment from backend deployment funding

    // Update treasury pool - only add developer payment, don't deduct deployment cost yet
    treasury_pool.distribute_fees(total_payment)?;

    emit!(DeploymentFundsRequested {
        request_id: deploy_request.request_id,
        developer: deploy_request.developer,
        program_hash: deploy_request.program_hash,
        service_fee,
        monthly_fee,
        initial_months,
        deployment_cost,
        total_payment,
        requested_at: current_time,
    });

    Ok(())
}

