pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use events::*;
#[allow(ambiguous_glob_reexports)]
pub use instructions::*;
pub use state::*;

declare_id!("5pAxXXdL7NzFKqpp6TnuxBojeFuKEijX6amRvY4G8dvA");

#[program]
pub mod nozz_launchpad {
    use super::*;

    // ********** Platform config **********

    /// Initialize the global platform config (admin only)
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        params: InitializeConfigParams,
    ) -> Result<()> {
        instructions::initialize_config(ctx, params)
    }

    /// Update the global platform config (admin only)
    pub fn update_config(ctx: Context<UpdateConfig>, params: UpdateConfigParams) -> Result<()> {
        instructions::update_config(ctx, params)
    }

    // *************************************

    // ********** Token LaunchPad **********

    /// Create a new streamer token with bonding curve
    pub fn create_token(ctx: Context<CreateToken>, params: CreateTokenParams) -> Result<()> {
        instructions::create_token(ctx, params)
    }

    /// Buy any streamer token with bonding curve
    pub fn buy_token(ctx: Context<BuyToken>, sol_amount: u64, min_tokens_out: u64) -> Result<()> {
        instructions::buy_token(ctx, sol_amount, min_tokens_out)
    }

    /// Sell any streamer token with bonding curve
    pub fn sell_token(ctx: Context<SellToken>, token_amount: u64, min_sol_out: u64) -> Result<()> {
        instructions::sell_token(ctx, token_amount, min_sol_out)
    }

    /// Claim streamer fees accumulated
    pub fn claim_fees(ctx: Context<ClaimFees>) -> Result<()> {
        instructions::claim_fees(ctx)
    }

    /// Graduate token to DEX once bonding curve is complete (permissionless)
    pub fn graduate_to_dex(ctx: Context<GraduateToDex>) -> Result<()> {
        instructions::graduate_to_dex(ctx)
    }

    // *************************************

    // ******** Stake-to-Subscribe ********

    /// Stake creator tokens to earn rewards and get subscriber status
    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        instructions::stake(ctx, amount)
    }

    /// Unstake tokens — subscriber status drops immediately if below threshold
    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        instructions::unstake(ctx, amount)
    }

    /// Claim accumulated staking rewards
    pub fn claim_stake_rewards(ctx: Context<ClaimStakeRewards>) -> Result<()> {
        instructions::claim_stake_rewards(ctx)
    }

    /// Creator can update minimum stake amount for subscriber status
    pub fn update_min_stake(ctx: Context<UpdateMinStake>, new_min_stake_amount: u64) -> Result<()> {
        instructions::update_min_stake(ctx, new_min_stake_amount)
    }

    // *************************************
}
