use crate::errors::ErrorCode;
use crate::events::DeploymentFundsRequested;
use crate::states::{DeployRequest, DeployRequestStatus, TreasuryPool, UserDeployStats};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_lang::solana_program::rent::Rent;

/// Create deploy request after payment verification
/// Only backend admin can call this instruction
/// Payment has already been verified and transferred to Reward Pool
/// This instruction creates the deploy_request and credits Reward Pool
#[derive(Accounts)]
#[instruction(program_hash: [u8; 32])]
pub struct CreateDeployRequest<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    /// CHECK: Reward Pool PDA (program-owned, receives monthly fee: 1% of borrowed amount)
    #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
    pub reward_pool: UncheckedAccount<'info>,
    
    /// CHECK: Platform Pool PDA (program-owned, receives platform fee: 0.1% of borrowed amount)
    #[account(
        mut,
        seeds = [TreasuryPool::PLATFORM_POOL_SEED],
        bump = treasury_pool.platform_pool_bump
    )]
    pub platform_pool: UncheckedAccount<'info>,
    
    /// CHECK: Deploy Request PDA - will be initialized/resized if needed
    /// We use UncheckedAccount to handle old layouts, then manually deserialize/resize
    #[account(
        mut,
        seeds = [DeployRequest::PREFIX_SEED, program_hash.as_ref()],
        bump
    )]
    pub deploy_request: UncheckedAccount<'info>,
    
    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + UserDeployStats::INIT_SPACE,
        seeds = [UserDeployStats::PREFIX_SEED, developer.key().as_ref()],
        bump
    )]
    pub user_stats: Account<'info, UserDeployStats>,
    
    /// CHECK: Developer wallet (not a signer, payment already verified)
    #[account(mut)]
    pub developer: UncheckedAccount<'info>,
    
    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn create_deploy_request(
    ctx: Context<CreateDeployRequest>,
    program_hash: [u8; 32],
    service_fee: u64,
    monthly_fee: u64,
    initial_months: u32,
    deployment_cost: u64,
) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request_info = ctx.accounts.deploy_request.to_account_info();
    let user_stats = &mut ctx.accounts.user_stats;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Handle deploy_request account (may have old layout)
    let required_space = 8 + DeployRequest::INIT_SPACE;
    let current_space = deploy_request_info.data_len();
    let is_new_account = current_space == 0;
    
    // Initialize account if new
    if is_new_account {
        let rent = Rent::get()?;
        let lamports_required = rent.minimum_balance(required_space);
        // Transfer lamports from admin to deploy_request account via CPI
        let transfer_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.admin.to_account_info(),
                to: deploy_request_info.clone(),
            },
        );
        system_program::transfer(transfer_cpi, lamports_required)?;
        deploy_request_info.realloc(required_space, false)?;
        let mut data = deploy_request_info.try_borrow_mut_data()?;
        data[..].fill(0);
    } else if current_space < required_space {
        // Resize account if old layout - need to add lamports for rent exemption
        msg!("[CREATE_DEPLOY_REQUEST] Resizing deploy_request from {} to {} bytes", current_space, required_space);
        
        let rent = Rent::get()?;
        let current_rent = rent.minimum_balance(current_space);
        let new_rent = rent.minimum_balance(required_space);
        let additional_lamports_needed = new_rent
            .checked_sub(current_rent)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        msg!("[CREATE_DEPLOY_REQUEST] Current rent: {} lamports, New rent: {} lamports", current_rent, new_rent);
        msg!("[CREATE_DEPLOY_REQUEST] Additional lamports needed: {} lamports", additional_lamports_needed);
        
        // Transfer additional lamports from admin to deploy_request account via CPI
        let transfer_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.admin.to_account_info(),
                to: deploy_request_info.clone(),
            },
        );
        system_program::transfer(transfer_cpi, additional_lamports_needed)?;
        
        deploy_request_info.realloc(required_space, false)?;
        // Zero out the new portion
        let mut data = deploy_request_info.try_borrow_mut_data()?;
        data[current_space..].fill(0);
    }
    
    // Deserialize deploy_request (will work after resize/init)
    let mut deploy_request = match DeployRequest::try_deserialize(&mut &deploy_request_info.data.borrow()[..]) {
        Ok(dr) => dr,
        Err(_) => {
            // If deserialization fails, initialize as new
            msg!("[CREATE_DEPLOY_REQUEST] Deserialization failed, initializing as new account");
            DeployRequest {
                request_id: [0u8; 32],
                developer: Pubkey::default(),
                program_hash: [0u8; 32],
                service_fee: 0,
                monthly_fee: 0,
                deployment_cost: 0,
                borrowed_amount: 0,
                subscription_paid_until: 0,
                ephemeral_key: None,
                deployed_program_id: None,
                status: DeployRequestStatus::PendingDeployment,
                created_at: 0,
                bump: ctx.bumps.deploy_request,
            }
        }
    };
    
    let is_new_deploy_request =
        deploy_request.request_id == [0u8; 32] && deploy_request.developer == Pubkey::default();

    // Assign bump
    deploy_request.bump = ctx.bumps.deploy_request;

    // Validation
    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(service_fee > 0, ErrorCode::InvalidAmount);
    require!(monthly_fee > 0, ErrorCode::InvalidAmount);
    require!(initial_months > 0, ErrorCode::InvalidAmount);
    require!(deployment_cost > 0, ErrorCode::InvalidAmount);

    // Note: Deployment cost funding will be handled by fund_temporary_wallet
    // We don't check pool balances here as funding comes from Admin/Reward Pool

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

    // Calculate total payment and fee breakdown
    // Payment structure:
    // - monthlyFee (1% monthly) + serviceFee → RewardPool
    // - deploymentPlatformFee (0.1% platform) → PlatformPool
    let monthly_fee_total = monthly_fee
        .checked_mul(initial_months as u64)
        .ok_or(ErrorCode::CalculationOverflow)?;
    let reward_fee_amount = monthly_fee_total
        .checked_add(service_fee)
        .ok_or(ErrorCode::CalculationOverflow)?; // Monthly fee + service fee → RewardPool
    let platform_fee_amount = deployment_cost
        .checked_div(1000)
        .ok_or(ErrorCode::CalculationOverflow)?; // 0.1% of deployment_cost → PlatformPool
    let total_payment = reward_fee_amount
        .checked_add(platform_fee_amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // Initialize deploy request with PendingDeployment status
    if is_new_deploy_request {
        deploy_request.request_id = program_hash;
        deploy_request.developer = ctx.accounts.developer.key();
        deploy_request.program_hash = program_hash;
        deploy_request.created_at = current_time;
    } else {
        // Ensure this PDA corresponds to the provided hash/developer
        let hash_matches = deploy_request.program_hash == program_hash;
        let developer_matches = deploy_request.developer == ctx.accounts.developer.key();
        
        if hash_matches && !developer_matches {
            // Conflict handling (same as before)
            let can_reset = matches!(
                deploy_request.status,
                DeployRequestStatus::Failed
                    | DeployRequestStatus::Cancelled
                    | DeployRequestStatus::Closed
                    | DeployRequestStatus::SubscriptionExpired
                    | DeployRequestStatus::Suspended
            ) || (
                deploy_request.status == DeployRequestStatus::PendingDeployment
                    && deploy_request.ephemeral_key.is_none()
            ) || (
                deploy_request.status == DeployRequestStatus::Active
            );
            
            require!(
                can_reset,
                ErrorCode::InvalidRequestId
            );
            
            // Reset the deploy_request for new developer
            deploy_request.request_id = program_hash;
            deploy_request.developer = ctx.accounts.developer.key();
            deploy_request.program_hash = program_hash;
            deploy_request.created_at = current_time;
            deploy_request.ephemeral_key = None;
            deploy_request.deployed_program_id = None;
        } else if !hash_matches {
            require!(
                hash_matches,
                ErrorCode::InvalidRequestId
            );
        } else {
            // Hash and developer match - allow update if status permits
            // Allow retry for:
            // 1. Failed/Cancelled/Closed deployments (obvious retry cases)
            // 2. PendingDeployment without ephemeral_key (initial request, can retry)
            // 3. PendingDeployment with ephemeral_key (deployment in progress but can retry if needed)
            // 4. Active status (same developer/hash, can update subscription or retry deployment)
            let can_retry = matches!(
                deploy_request.status,
                DeployRequestStatus::Failed
                    | DeployRequestStatus::Cancelled
                    | DeployRequestStatus::Closed
                    | DeployRequestStatus::PendingDeployment
                    | DeployRequestStatus::Active
            ) || matches!(
                deploy_request.status,
                DeployRequestStatus::SubscriptionExpired
                    | DeployRequestStatus::Suspended
            );
            
            require!(
                can_retry,
                ErrorCode::InvalidDeploymentStatus
            );
        }
    }

    deploy_request.service_fee = service_fee;
    deploy_request.monthly_fee = monthly_fee;
    deploy_request.deployment_cost = deployment_cost;
    deploy_request.borrowed_amount = 0; // Will be set when temporary wallet is funded (equals deployment_cost)
    deploy_request.subscription_paid_until =
        current_time + (initial_months as i64 * 30 * 24 * 60 * 60);
    deploy_request.ephemeral_key = None; // Will be set when backend funds temporary wallet
    deploy_request.deployed_program_id = None; // Will be set after backend deploys
    deploy_request.status = DeployRequestStatus::PendingDeployment;

    // Update user stats
    user_stats.active_sessions += 1;
    user_stats.daily_deploys += 1;
    user_stats.total_deploys += 1;

    // IMPORTANT: Credit fees to pools
    // Note: Payment has already been transferred to pools by developer (off-chain):
    // - monthlyFee (1% monthly) + serviceFee → RewardPool
    // - deploymentPlatformFee (0.1% platform) → PlatformPool
    // We just need to update the state to track the balances
    
    // Credit fees to respective pools
    treasury_pool.credit_reward_pool(reward_fee_amount as u128)?;
    treasury_pool.credit_platform_pool(platform_fee_amount as u128)?;
    
    // Update reward_per_share if there are deposits
    if treasury_pool.total_deposited > 0 {
        // Only update reward_per_share for reward fees (not platform fees)
        let reward_per_share_increment = (reward_fee_amount as u128)
            .checked_mul(TreasuryPool::PRECISION)
            .and_then(|x| x.checked_div(treasury_pool.total_deposited as u128))
            .ok_or(ErrorCode::CalculationOverflow)?;
        treasury_pool.reward_per_share = treasury_pool
            .reward_per_share
            .checked_add(reward_per_share_increment)
            .ok_or(ErrorCode::CalculationOverflow)?;
    }
    
    // Verify pools have received the payments
    // This is a safety check - the actual transfers happened off-chain
    let reward_pool_lamports = ctx.accounts.reward_pool.lamports();
    let platform_pool_lamports = ctx.accounts.platform_pool.lamports();
    require!(
        reward_pool_lamports >= treasury_pool.reward_pool_balance,
        ErrorCode::InsufficientTreasuryFunds
    );
    require!(
        platform_pool_lamports >= treasury_pool.platform_pool_balance,
        ErrorCode::InsufficientTreasuryFunds
    );

    // Serialize deploy_request back to account
    deploy_request.try_serialize(&mut &mut deploy_request_info.data.borrow_mut()[..])?;
    
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
