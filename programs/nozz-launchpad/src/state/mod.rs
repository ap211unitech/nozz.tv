use anchor_lang::prelude::*;

/// Global platform configuration (singleton PDA)
#[account]
#[derive(InitSpace)]
pub struct NozzLaunchpadConfig {
    /// Admin authority - can update config
    pub authority: Pubkey,

    /// Treasury wallet that collects platform fees
    pub fee_recipient: Pubkey,

    /// Platform fee in basis points (e.g., 25 = 0.25%)
    pub platform_fee_bps: u16,

    /// Streamer fee in basis points earned on each trade (e.g., 75 = 0.75%)
    pub streamer_fee_bps: u16,

    /// Total supply for each new token (default: 1_000_000_000 with 6 decimals)
    pub initial_token_supply: u64,

    /// % of supply allocated to bonding curve (e.g., 40 = 40%)
    pub bonding_curve_supply_pct: u8,

    /// SOL threshold that triggers graduation (lamports).
    /// Graduation fires when EITHER this is hit OR all bonding curve supply (40%) tokens sell out.
    /// Example: 85_000_000_000 = 85 SOL
    pub graduation_sol_threshold: u64,

    /// Number of tokens created on platform
    pub token_count: u64,

    pub bump: u8,
}

impl NozzLaunchpadConfig {
    pub const SEED: &'static [u8] = b"nozz_launchpad_config";
    pub const LEN: usize = NozzLaunchpadConfig::INIT_SPACE;
}
