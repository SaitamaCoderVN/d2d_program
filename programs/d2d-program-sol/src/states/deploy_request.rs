use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum DeployRequestStatus {
    PendingDeployment,   // Payment made, waiting for deployment
    Active,              // Active with valid subscription
    SubscriptionExpired, // Subscription expired
    Suspended,           // Suspended due to non-payment
    Failed,              // Deployment failed
    Cancelled,           // Cancelled by developer
    Closed,              // Program closed, lamports recovered
}

#[account]
#[derive(InitSpace)]
pub struct DeployRequest {
    pub request_id: [u8; 32],                // Unique request identifier
    pub developer: Pubkey,                   // Developer public key
    pub program_hash: [u8; 32],              // Hash of program to deploy
    pub service_fee: u64,                    // One-time service fee
    pub monthly_fee: u64,                    // Monthly subscription fee
    pub deployment_cost: u64,                // Actual deployment cost from treasury
    pub borrowed_amount: u64,                // Amount borrowed from treasury (for fee calculation: 1% monthly)
    pub subscription_paid_until: i64,        // Subscription valid until timestamp
    pub ephemeral_key: Option<Pubkey>,       // Temporary key for deployment
    pub deployed_program_id: Option<Pubkey>, // Deployed program ID
    pub status: DeployRequestStatus,         // Current status
    pub created_at: i64,                     // Creation timestamp
    pub bump: u8,                            // PDA bump
}

impl DeployRequest {
    pub const PREFIX_SEED: &'static [u8] = b"deploy_request";

    pub fn is_subscription_valid(&self) -> Result<bool> {
        let current_time = Clock::get()?.unix_timestamp;
        Ok(current_time <= self.subscription_paid_until)
    }

    pub fn extend_subscription(&mut self, months: u32) {
        let seconds_per_month = 30 * 24 * 60 * 60; // 30 days
        let extension_seconds = months as i64 * seconds_per_month;
        self.subscription_paid_until += extension_seconds;
    }
}
