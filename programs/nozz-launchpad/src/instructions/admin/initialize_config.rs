use anchor_lang::prelude::*;

use crate::{error::NozzError, state::NozzLaunchpadConfig, CREATOR_TOKEN_MINT_DECIMALS};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeConfigParams {
    /// Treasury that receives platform fees
    pub treasury: Pubkey,

    /// Platform fee bps (e.g. 25 = 0.25%)
    pub platform_fee_bps: u16,

    /// Creator/streamer fee bps (e.g. 75 = 0.75%)
    pub streamer_fee_bps: u16,

    /// Total token supply per launch (default: 1_000_000_000_000_000 with 6 decimals)
    pub initial_token_supply: u64,

    /// SOL in vault that triggers graduation (lamports).
    /// Works alongside token sellout — whichever fires first wins.
    /// Recommended: 85_000_000_000 (85 SOL)
    pub graduation_sol_threshold: u64,

    /// % of supply for bonding curve trading (rest goes to DEX liquidity)
    pub bonding_curve_supply_pct: u8,
}

pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    params: InitializeConfigParams,
) -> Result<()> {
    require!(params.platform_fee_bps <= 1000, NozzError::InvalidFee);
    require!(params.streamer_fee_bps <= 1000, NozzError::InvalidFee);
    require!(
        params.bonding_curve_supply_pct > 0 && params.bonding_curve_supply_pct <= 100,
        NozzError::InvalidFee
    );
    require!(params.graduation_sol_threshold > 0, NozzError::ZeroAmount);

    let token_decimals_factor: u64 = (10 as u64).pow(CREATOR_TOKEN_MINT_DECIMALS as u32);
    let total_supply = params
        .initial_token_supply
        .checked_mul(token_decimals_factor)
        .ok_or(NozzError::MathOverflow)?;

    let config = &mut ctx.accounts.nozz_launchpad_config;
    config.authority = ctx.accounts.authority.key();
    config.treasury = params.treasury;
    config.platform_fee_bps = params.platform_fee_bps;
    config.streamer_fee_bps = params.streamer_fee_bps;
    config.graduation_sol_threshold = params.graduation_sol_threshold;
    config.initial_token_supply = total_supply;
    config.bonding_curve_supply_pct = params.bonding_curve_supply_pct;
    config.token_count = 0;
    config.bump = ctx.bumps.nozz_launchpad_config;

    msg!(
        "Nozz Launchpad initialized | graduation threshold: {} lamports",
        params.graduation_sol_threshold
    );
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = NozzLaunchpadConfig::LEN,
        seeds = [NozzLaunchpadConfig::SEED],
        bump
    )]
    pub nozz_launchpad_config: Account<'info, NozzLaunchpadConfig>,

    pub system_program: Program<'info, System>,
}
