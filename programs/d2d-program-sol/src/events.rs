use anchor_lang::prelude::*;

#[event]
pub struct TreasuryInitialized {
    pub admin: Pubkey,
    pub treasury_wallet: Pubkey,
    pub initial_apy: u64,
}

#[event]
pub struct SolStaked {
    pub lender: Pubkey,
    pub amount: u64,
    pub total_staked: u64,
    pub lock_period: i64,
}

#[event]
pub struct SolUnstaked {
    pub lender: Pubkey,
    pub amount: u64,
    pub remaining_staked: u64,
}

#[event]
pub struct RewardsClaimed {
    pub lender: Pubkey,
    pub amount: u64,
    pub total_claimed: u64,
}

#[event]
pub struct DeployRequested {
    pub request_id: [u8; 32],
    pub developer: Pubkey,
    pub program_hash: [u8; 32],
    pub service_fee: u64,
    pub monthly_fee: u64,
    pub initial_months: u32,
    pub total_payment: u64,
}

#[event]
pub struct DeploymentFundsRequested {
    pub request_id: [u8; 32],
    pub developer: Pubkey,
    pub program_hash: [u8; 32],
    pub service_fee: u64,
    pub monthly_fee: u64,
    pub initial_months: u32,
    pub deployment_cost: u64,
    pub total_payment: u64,
    pub requested_at: i64,
}

#[event]
pub struct TemporaryWalletFunded {
    pub request_id: [u8; 32],
    pub temporary_wallet: Pubkey,
    pub amount: u64,
    pub funded_at: i64,
}

#[event]
pub struct ProgramDeployed {
    pub request_id: [u8; 32],
    pub developer: Pubkey,
    pub program_hash: [u8; 32],
    pub service_fee: u64,
    pub monthly_fee: u64,
    pub initial_months: u32,
    pub deployment_cost: u64,
    pub ephemeral_key: Pubkey,
    pub total_payment: u64,
    pub deployed_at: i64,
}

#[event]
pub struct DeploymentConfirmed {
    pub request_id: [u8; 32],
    pub developer: Pubkey,
    pub deployed_program_id: Pubkey,
    pub deployment_cost: u64,
    pub recovered_funds: u64,
    pub confirmed_at: i64,
}

#[event]
pub struct DeploymentFailed {
    pub request_id: [u8; 32],
    pub developer: Pubkey,
    pub failure_reason: String,
    pub refund_amount: u64,
    pub deployment_cost_returned: u64,
    pub failed_at: i64,
}

#[event]
pub struct SubscriptionPaid {
    pub request_id: [u8; 32],
    pub developer: Pubkey,
    pub months: u32,
    pub payment_amount: u64,
    pub subscription_valid_until: i64,
}

#[event]
pub struct RewardsDistributed {
    pub total_fees_collected: u64,
    pub total_rewards_distributed: u64,
    pub current_apy: u64,
    pub distributed_at: i64,
}

#[event]
pub struct ApyUpdated {
    pub old_apy: u64,
    pub new_apy: u64,
    pub updated_at: i64,
}

#[event]
pub struct ProgramsSuspended {
    pub suspended_count: u32,
    pub suspended_at: i64,
}

#[event]
pub struct EmergencyPauseToggled {
    pub paused: bool,
    pub toggled_at: i64,
}

#[event]
pub struct ProgramClosed {
    pub request_id: [u8; 32],
    pub program_id: Pubkey,
    pub developer: Pubkey,
    pub recovered_lamports: u64,
    pub closed_at: i64,
}

#[event]
pub struct AdminWithdrew {
    pub admin: Pubkey,
    pub amount: u64,
    pub destination: Pubkey,
    pub reason: String,
    pub withdrawn_at: i64,
}

#[event]
pub struct AdminMovedToRewardPool {
    pub admin: Pubkey,
    pub amount: u64,
    pub moved_at: i64,
}

#[event]
pub struct DepositMade {
    pub backer: Pubkey,
    pub deposit_amount: u64,
    pub net_deposit: u64,
    pub reward_fee: u64,
    pub platform_fee: u64,
    pub total_deposited: u64,
    pub liquid_balance: u64,
    pub deposited_at: i64,
}

#[event]
pub struct RewardCredited {
    pub fee_reward: u64,
    pub fee_platform: u64,
    pub reward_per_share: u128,
    pub total_deposited: u64,
    pub credited_at: i64,
}

#[event]
pub struct Claimed {
    pub backer: Pubkey,
    pub amount: u64,
    pub claimed_total: u64,
    pub reward_per_share: u128,
    pub claimed_at: i64,
}

#[event]
pub struct WithdrawRequested {
    pub backer: Pubkey,
    pub amount: u64,
    pub request_id: [u8; 32],
    pub requested_at: i64,
}
