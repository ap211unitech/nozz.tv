use anchor_lang::prelude::*;

use crate::ANCHOR_DISCRIMINATOR;

/// Global platform configuration (singleton PDA)
#[account]
#[derive(InitSpace, Debug)]
pub struct NozzLaunchpadConfig {
    /// Admin authority - can update config
    pub authority: Pubkey,

    /// Treasury wallet that collects platform fees
    pub fee_recipient: Pubkey,

    /// Platform fee in basis points (e.g., 25 = 0.25%)
    pub platform_fee_bps: u16,

    /// Streamer fee in basis points earned on each trade (e.g., 75 = 0.75%)
    pub streamer_fee_bps: u16,

    /// Total supply for each new token (default: 100 million * (10 ** decimals)) (with decimals)
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
    pub const LEN: usize = (ANCHOR_DISCRIMINATOR as usize) + NozzLaunchpadConfig::INIT_SPACE;
}

/// Per-token bonding curve state
#[account]
#[derive(InitSpace)]
pub struct BondingCurve {
    /// The SPL mint for this token
    pub mint: Pubkey,

    /// The streamer/creator who launched this token
    pub creator: Pubkey,

    /// Virtual SOL reserves — includes the seeded virtual SOL for price bootstrapping.
    /// Starts at VIRTUAL_SOL_SEED (30 SOL) and grows with each buy.
    pub virtual_sol_reserves: u64,

    /// Virtual token reserves — the 40% bonding curve allocation.
    /// Decreases with each buy, increases with each sell.
    pub virtual_token_reserves: u64,

    /// Real SOL deposited by actual buyers (does NOT include virtual seed).
    /// This is what gets migrated to Raydium on graduation.
    pub real_sol_reserves: u64,

    /// Real tokens still available for purchase in the bonding curve.
    /// Starts at bonding_curve_allocation, hits 0 when fully sold out.
    pub real_token_reserves: u64,

    /// Total token supply minted (raw, before decimals)
    pub total_supply: u64,

    /// Tokens allocated to bonding curve trading (40% of supply, raw)
    pub bonding_curve_allocation: u64,

    /// SOL threshold snapshot copied from config at token creation time.
    /// Graduation fires when real_sol_reserves >= this OR real_token_reserves == 0.
    pub graduation_sol_threshold: u64,

    /// True once graduation condition is met.
    /// Trading stops and graduate_to_dex becomes callable.
    pub complete: bool,

    /// True once graduate_to_dex has executed successfully.
    /// Prevents double-migration.
    pub migrated: bool,

    /// Streamer fees accumulated (lamports), claimable anytime via claim_fees
    pub pending_creator_fees: u64,

    /// Unix timestamp of creation
    pub created_at: i64,

    /// Total volume traded through bonding curve (lamports)
    pub total_volume: u64,

    pub bump: u8,
    pub vault_bump: u8,
}

impl BondingCurve {
    pub const CREATOR_TOKEN_MINT_SEED: &'static [u8] = b"creator_token_mint";
    pub const SEED: &'static [u8] = b"bonding_curve";
    pub const VAULT_SEED: &'static [u8] = b"bonding_curve_vault";
    pub const LEN: usize = (ANCHOR_DISCRIMINATOR as usize) + BondingCurve::INIT_SPACE;

    /// Hybrid graduation check:
    /// completes when SOL threshold is hit OR all bonding curve tokens sold
    pub fn should_graduate(&self) -> bool {
        self.real_sol_reserves >= self.graduation_sol_threshold || self.real_token_reserves == 0
    }
}
