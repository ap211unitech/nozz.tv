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

    /// % of supply for bonding curve trading
    pub bonding_curve_supply_pct: u8,

    /// % of supply allocated to staking reward pool
    pub staking_supply_pct: u8,

    /// % for DEX liquidity on graduation
    /// Must satisfy: bonding_curve + staking + dex == 100
    pub dex_supply_pct: u8,

    /// Reward emission duration in seconds (default: 157_680_000 = 5 years)
    pub staking_duration_seconds: u64,
}

pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    params: InitializeConfigParams,
) -> Result<()> {
    require!(params.platform_fee_bps <= 1000, NozzError::InvalidFee);
    require!(params.streamer_fee_bps <= 1000, NozzError::InvalidFee);
    require!(params.graduation_sol_threshold > 0, NozzError::ZeroAmount);
    require!(
        params.bonding_curve_supply_pct > 0 && params.bonding_curve_supply_pct <= 100,
        NozzError::InvalidFee
    );
    require!(
        (params.bonding_curve_supply_pct as u16)
            .checked_add(params.staking_supply_pct as u16)
            .and_then(|s| s.checked_add(params.dex_supply_pct as u16))
            == Some(100),
        NozzError::InvalidSupplyAllocation
    );

    let token_decimals_factor: u64 = (10 as u64).pow(CREATOR_TOKEN_MINT_DECIMALS as u32);
    let total_supply = params
        .initial_token_supply
        .checked_mul(token_decimals_factor)
        .ok_or(NozzError::MathOverflow)?;

    ctx.accounts
        .nozz_launchpad_config
        .set_inner(NozzLaunchpadConfig {
            authority: ctx.accounts.authority.key(),
            treasury: params.treasury,
            platform_fee_bps: params.platform_fee_bps,
            streamer_fee_bps: params.streamer_fee_bps,
            graduation_sol_threshold: params.graduation_sol_threshold,
            initial_token_supply: total_supply,
            bonding_curve_supply_pct: params.bonding_curve_supply_pct,
            staking_supply_pct: params.staking_supply_pct,
            dex_supply_pct: params.dex_supply_pct,
            staking_duration_seconds: params.staking_duration_seconds,
            token_count: 0,
            bump: ctx.bumps.nozz_launchpad_config,
        });

    msg!(
        "Nozz Launchpad initialized | BC: {}% | Staking: {}% | DEX: {}% | graduation: {} lamports",
        params.bonding_curve_supply_pct,
        params.staking_supply_pct,
        params.dex_supply_pct,
        params.graduation_sol_threshold,
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
