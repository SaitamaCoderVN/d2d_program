use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserDeployStats {
    pub user: Pubkey,         // User public key
    pub active_sessions: u32, // Current active sessions
    pub daily_deploys: u32,   // Daily deploy count
    pub total_deploys: u64,   // Total deployments
    pub last_reset: i64,      // Last daily reset timestamp
    pub bump: u8,             // PDA bump
}

impl UserDeployStats {
    pub const PREFIX_SEED: &'static [u8] = b"user_stats";
}
