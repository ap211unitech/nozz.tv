use anchor_lang::{prelude::*, system_program};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::{transfer_checked, Token2022, TransferChecked},
    token_interface::{Mint, TokenAccount},
};

use crate::{
    error::NozzError,
    utils::{calculate_fee, get_tokens_for_sol},
    BondingCurve, NozzLaunchpadConfig, TradeEvent, CREATOR_TOKEN_MINT_DECIMALS,
};

pub fn buy_token(ctx: Context<BuyToken>, sol_amount: u64, min_tokens_out: u64) -> Result<()> {
    require!(sol_amount > 0, NozzError::ZeroAmount);

    let clock = Clock::get()?;
    let config = &ctx.accounts.nozz_launchpad_config;
    let bonding_curve = &ctx.accounts.bonding_curve;
    let token_mint = &ctx.accounts.mint;

    // fees
    let platform_fee = calculate_fee(sol_amount, config.platform_fee_bps)?;
    let creator_fee = calculate_fee(sol_amount, config.streamer_fee_bps)?;
    let total_fees = platform_fee
        .checked_add(creator_fee)
        .ok_or(NozzError::MathOverflow)?;
    let sol_after_fees = sol_amount
        .checked_sub(total_fees)
        .ok_or(NozzError::MathOverflow)?;

    // bonding curve math
    let tokens_out = get_tokens_for_sol(
        bonding_curve.virtual_sol_reserves,
        bonding_curve.virtual_token_reserves,
        sol_after_fees,
    )?;

    // slippage control
    require!(tokens_out >= min_tokens_out, NozzError::SlippageExceeded);
    require!(
        tokens_out <= bonding_curve.real_token_reserves,
        NozzError::InsufficientTokens
    );

    // Transfer SOL: buyer → platform treasury
    if platform_fee > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            platform_fee,
        )?;
    }

    // Transfer SOL: buyer → bonding curve vault (for SOL)
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.bonding_curve_sol_vault.to_account_info(),
            },
        ),
        sol_after_fees,
    )?;

    // Transfer Tokens: bonding_curve ATA → buyer ATA (TransferChecked for Token-2022)
    // TransferChecked is required for Token-2022 — it validates decimals
    // and handles any transfer hook extensions on the mint.
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                authority: ctx.accounts.bonding_curve.to_account_info(),
                from: ctx.accounts.bonding_curve_token_vault.to_account_info(),
                to: ctx.accounts.buyer_token_ata.to_account_info(),
                mint: token_mint.to_account_info(),
            },
            &[&[
                BondingCurve::SEED,
                ctx.accounts.bonding_curve.mint.as_ref(),
                &[bonding_curve.vault_bump],
            ]],
        ),
        tokens_out,
        token_mint.decimals,
    )?;

    // Update state
    let bonding_curve = &mut ctx.accounts.bonding_curve;
    bonding_curve.virtual_sol_reserves = bonding_curve
        .virtual_sol_reserves
        .checked_add(sol_after_fees)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.virtual_token_reserves = bonding_curve
        .virtual_token_reserves
        .checked_sub(tokens_out)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.real_sol_reserves = bonding_curve
        .real_sol_reserves
        .checked_add(sol_after_fees)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.real_token_reserves = bonding_curve
        .real_token_reserves
        .checked_sub(tokens_out)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.claimable_creator_fees = bonding_curve
        .claimable_creator_fees
        .checked_add(creator_fee)
        .ok_or(NozzError::MathOverflow)?;
    bonding_curve.total_volume = bonding_curve
        .total_volume
        .checked_add(sol_amount)
        .ok_or(NozzError::MathOverflow)?;

    // Hybrid graduation check
    if bonding_curve.should_graduate() {
        bonding_curve.complete = true;
        let reason = if bonding_curve.real_sol_reserves >= bonding_curve.graduation_sol_threshold {
            "SOL threshold reached"
        } else {
            "all bonding curve tokens sold"
        };
        msg!("Bonding curve complete ({})! Call graduate_to_dex.", reason);
    }

    emit!(TradeEvent {
        mint: bonding_curve.mint,
        trader: ctx.accounts.buyer.key(),
        is_buy: true,
        sol_amount,
        token_amount: tokens_out,
        platform_fee,
        creator_fee,
        virtual_sol_reserves: bonding_curve.virtual_sol_reserves,
        virtual_token_reserves: bonding_curve.virtual_token_reserves,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct BuyToken<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        seeds = [NozzLaunchpadConfig::SEED],
        bump = nozz_launchpad_config.bump
    )]
    pub nozz_launchpad_config: Account<'info, NozzLaunchpadConfig>,

    #[account(
        mut,
        constraint = treasury.key() == nozz_launchpad_config.treasury @ NozzError::InavlidTreasury
    )]
    /// CHECK: Verified against config
    pub treasury: UncheckedAccount<'info>,

    #[account(
        mint::decimals = CREATOR_TOKEN_MINT_DECIMALS,
        mint::authority = bonding_curve,
        mint::freeze_authority = bonding_curve,
        mint::token_program = token_program,
        extensions::metadata_pointer::authority = bonding_curve,
        extensions::metadata_pointer::metadata_address = mint,
        seeds = [BondingCurve::CREATOR_TOKEN_MINT_SEED, bonding_curve.creator.key().as_ref()],
        bump,
        constraint = mint.key() == bonding_curve.mint @ NozzError::InvalidTokenMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [BondingCurve::SEED, bonding_curve.mint.key().as_ref()],
        bump = bonding_curve.bump,
        constraint = !bonding_curve.complete @ NozzError::BondingCurveComplete,
        constraint = !bonding_curve.migrated @ NozzError::AlreadyGraduated,
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    #[account(
        seeds = [BondingCurve::VAULT_SEED, mint.key().as_ref()],
        bump = bonding_curve.vault_bump
    )]
    /// CHECK: PDA used as SOL vault
    pub bonding_curve_sol_vault: UncheckedAccount<'info>,

    #[account(
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
        associated_token::token_program = token_program
    )]
    pub bonding_curve_token_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = buyer,
        associated_token::token_program = token_program
    )]
    pub buyer_token_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
