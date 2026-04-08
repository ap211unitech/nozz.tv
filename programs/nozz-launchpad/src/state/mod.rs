use anchor_lang::prelude::*;

use crate::ANCHOR_DISCRIMINATOR;

/// Global platform configuration (singleton PDA)
#[account]
#[derive(InitSpace, Debug)]
pub struct NozzLaunchpadConfig {
    /// Admin authority - can update config
    pub authority: Pubkey,

    /// Treasury wallet that collects platform fees
    pub treasury: Pubkey,

    /// Platform fee in basis points (e.g., 25 = 0.25%)
    pub platform_fee_bps: u16,

    /// Streamer fee in basis points earned on each trade (e.g., 75 = 0.75%)
    pub streamer_fee_bps: u16,

    /// Total supply for each new token (default: 100 million * (10 ** decimals)) (with decimals)
    pub initial_token_supply: u64,

    /// % of supply allocated to bonding curve (e.g., 30 = 30%)
    pub bonding_curve_supply_pct: u8,

    /// % of supply allocated to staking reward pool (e.g., 40 = 40%)
    pub staking_supply_pct: u8,

    /// % for DEX liquidity on graduation (e.g. 30)
    /// Must satisfy: bonding_curve + staking + dex == 100
    pub dex_supply_pct: u8,

    /// Reward emission duration in seconds (default: 157_680_000 = 5 years)
    pub staking_duration_seconds: u64,

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
    pub claimable_creator_fees: u64,

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

/// Per-token staking pool state
#[account]
#[derive(InitSpace)]
pub struct StakePool {
    /// The token mint this pool is for
    pub mint: Pubkey,

    /// Creator of the token — only they can update min_stake_amount
    pub creator: Pubkey,

    /// Minimum tokens a viewer must stake to get subscriber status
    pub min_stake_amount: u64,

    /// Total tokens currently staked across all stakers
    pub total_staked: u64,

    /// Tokens emitted per second from the reward pool
    /// = staking_allocation / staking_duration_seconds
    pub reward_rate_per_second: u64,

    /// Global accumulator: reward tokens earned per staked token since inception
    /// Scaled by REWARD_SCALE to preserve precision (avoids fractional tokens)
    pub reward_per_token_stored: u128,

    /// Timestamp of last time reward_per_token_stored was updated
    pub last_update_time: i64,

    /// Total rewards distributed so far (for accounting / frontend)
    pub total_rewards_distributed: u64,

    /// Remaining tokens in reward vault (decrements as rewards are claimed)
    pub reward_vault_balance: u64,

    /// Emission end time: created_at + staking_duration_seconds
    pub reward_end_time: i64,

    pub bump: u8,
}

impl StakePool {
    pub const SEED: &'static [u8] = b"stake_pool";
    pub const REWARD_VAULT_SEED: &'static [u8] = b"stake_reward_vault";

    /// Scale factor to preserve precision in reward_per_token_stored
    /// Using 1e12 means we can represent fractions down to 1/1_000_000_000_000
    pub const REWARD_SCALE: u128 = 1_000_000_000_000;

    pub const LEN: usize = (ANCHOR_DISCRIMINATOR as usize) + StakePool::INIT_SPACE;
}

/// Per-user per-token stake position
#[account]
#[derive(InitSpace)]
pub struct StakePosition {
    /// Wallet that owns this stake
    pub owner: Pubkey,

    /// The token mint staked
    pub mint: Pubkey,

    /// Amount of tokens currently staked
    pub amount_staked: u64,

    /// Snapshot of reward_per_token_stored when user last interacted
    /// Used to calculate pending rewards since last interaction
    pub reward_per_token_paid: u128,

    /// Rewards accumulated but not yet claimed (raw token units)
    pub rewards_earned: u64,

    /// True if amount_staked >= stake_pool.min_stake_amount
    pub is_subscribed: bool,

    pub staked_at: i64,
    pub bump: u8,
}

impl StakePosition {
    pub const SEED: &'static [u8] = b"stake_position";
    pub const LEN: usize = (ANCHOR_DISCRIMINATOR as usize) + StakePosition::INIT_SPACE;
}
