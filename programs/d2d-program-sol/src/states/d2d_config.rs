use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct D2DConfig {
    pub admin: Pubkey,                // Program admin
    pub treasury: Pubkey,             // Treasury wallet
    pub fee_rate: u64,                // Fee in lamports
    pub max_concurrent_per_user: u32, // Max concurrent sessions per user
    pub total_deploys: u64,           // Total successful deployments
    pub total_fees_collected: u64,    // Total fees collected
    pub is_paused: bool,              // Emergency pause flag
    pub bump: u8,                     // PDA bump
}

impl D2DConfig {
    pub const PREFIX_SEED: &'static [u8] = b"d2d_config";
}
