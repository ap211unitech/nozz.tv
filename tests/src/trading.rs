use anchor_client::{
    anchor_lang::prelude::system_program,
    solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction},
};
use nozz_launchpad::{
    accounts as nozz_accounts, instruction as nozz_instructions, state as nozz_state, utils,
    CreateTokenParams,
};
use serial_test::serial;

use crate::utils::{send_initialize_config, setup_environment, Environment};

fn associated_token_address(owner: Pubkey, mint: Pubkey, token_program: Pubkey) -> Pubkey {
    spl_associated_token_account::get_associated_token_address_with_program_id(
        &owner.to_bytes().into(),
        &mint.to_bytes().into(),
        &token_program.to_bytes().into(),
    )
    .to_bytes()
    .into()
}

#[allow(deprecated)]
fn system_transfer(
    from: &Pubkey,
    to: &Pubkey,
    lamports: u64,
) -> anchor_client::solana_sdk::instruction::Instruction {
    anchor_client::solana_sdk::system_instruction::transfer(from, to, lamports)
}

#[test]
#[serial]
fn test_buy_and_stake_updates_state_without_graduating() {
    let Environment {
        program,
        payer,
        config_pda,
        ..
    } = setup_environment();

    send_initialize_config();

    let config: nozz_state::NozzLaunchpadConfig = program.account(config_pda).unwrap();
    let creator = Keypair::new();
    let blockhash = program.rpc().get_latest_blockhash().unwrap();
    let fund_creator = Transaction::new_signed_with_payer(
        &[system_transfer(
            &payer.pubkey(),
            &creator.pubkey(),
            2_000_000_000,
        )],
        Some(&payer.pubkey()),
        &[payer.as_ref()],
        blockhash,
    );
    program
        .rpc()
        .send_and_confirm_transaction(&fund_creator)
        .unwrap();

    let mint_pubkey = Pubkey::find_program_address(
        &[
            nozz_state::BondingCurve::CREATOR_TOKEN_MINT_SEED,
            creator.pubkey().as_ref(),
        ],
        &nozz_launchpad::ID,
    )
    .0;
    let bonding_curve_pda = Pubkey::find_program_address(
        &[nozz_state::BondingCurve::SEED, mint_pubkey.as_ref()],
        &nozz_launchpad::ID,
    )
    .0;
    let bonding_curve_vault_pda = Pubkey::find_program_address(
        &[nozz_state::BondingCurve::VAULT_SEED, mint_pubkey.as_ref()],
        &nozz_launchpad::ID,
    )
    .0;
    let bonding_curve_ata = associated_token_address(
        bonding_curve_pda,
        mint_pubkey,
        spl_token_2022::id().to_bytes().into(),
    );
    let stake_pool_pda = Pubkey::find_program_address(
        &[nozz_state::StakePool::SEED, mint_pubkey.as_ref()],
        &nozz_launchpad::ID,
    )
    .0;
    let stake_reward_vault = associated_token_address(
        stake_pool_pda,
        mint_pubkey,
        spl_token_2022::id().to_bytes().into(),
    );
    program
        .request()
        .accounts(nozz_accounts::CreateToken {
            creator: creator.pubkey(),
            nozz_launchpad_config: config_pda,
            mint: mint_pubkey,
            bonding_curve: bonding_curve_pda,
            bonding_curve_vault: bonding_curve_vault_pda,
            bonding_curve_ata,
            stake_pool: stake_pool_pda,
            stake_reward_vault,
            token_program: spl_token_2022::id().to_bytes().into(),
            associated_token_program: spl_associated_token_account::id().to_bytes().into(),
            system_program: system_program::ID,
            rent: anchor_client::solana_sdk::sysvar::rent::ID,
        })
        .args(nozz_instructions::CreateToken {
            params: CreateTokenParams {
                token_name: "NozzTradingTest".to_string(),
                token_ticker: "NZT".to_string(),
                token_uri: "https://example.com/nozz-trading-test.json".to_string(),
                min_stake_amount: 10,
            },
        })
        .signer(&creator)
        .send()
        .unwrap();

    let curve_before: nozz_state::BondingCurve = program.account(bonding_curve_pda).unwrap();
    let treasury_balance = program.rpc().get_balance(&config.treasury).unwrap();
    if treasury_balance < 2_000_000_000 {
        let transfer = system_transfer(
            &payer.pubkey(),
            &config.treasury,
            2_000_000_000 - treasury_balance,
        );
        let blockhash = program.rpc().get_latest_blockhash().unwrap();
        let transaction = Transaction::new_signed_with_payer(
            &[transfer],
            Some(&payer.pubkey()),
            &[payer.as_ref()],
            blockhash,
        );
        program
            .rpc()
            .send_and_confirm_transaction(&transaction)
            .unwrap();
    }
    assert!(program.rpc().get_balance(&config.treasury).unwrap() >= 2_000_000_000);
    let buyer_token_ata = associated_token_address(
        payer.pubkey(),
        mint_pubkey,
        spl_token_2022::id().to_bytes().into(),
    );
    let sol_amount = 100_000_000;
    let platform_fee = utils::calculate_fee(sol_amount, config.platform_fee_bps).unwrap();
    let creator_fee = utils::calculate_fee(sol_amount, config.streamer_fee_bps).unwrap();
    let net_sol = sol_amount - platform_fee - creator_fee;
    let tokens_out = utils::get_tokens_for_sol(
        curve_before.virtual_sol_reserves,
        curve_before.virtual_token_reserves,
        net_sol,
    )
    .unwrap();

    program
        .request()
        .accounts(nozz_accounts::BuyToken {
            buyer: payer.pubkey(),
            nozz_launchpad_config: config_pda,
            treasury: config.treasury,
            mint: mint_pubkey,
            bonding_curve: bonding_curve_pda,
            bonding_curve_sol_vault: bonding_curve_vault_pda,
            bonding_curve_token_vault: bonding_curve_ata,
            buyer_token_ata,
            token_program: spl_token_2022::id().to_bytes().into(),
            associated_token_program: spl_associated_token_account::id().to_bytes().into(),
            system_program: system_program::ID,
        })
        .args(nozz_instructions::BuyToken {
            sol_amount,
            min_tokens_out: tokens_out,
        })
        .send()
        .unwrap();

    let curve_after_buy: nozz_state::BondingCurve = program.account(bonding_curve_pda).unwrap();
    assert_eq!(
        curve_after_buy.virtual_sol_reserves,
        curve_before.virtual_sol_reserves + net_sol
    );
    assert_eq!(
        curve_after_buy.virtual_token_reserves,
        curve_before.virtual_token_reserves - tokens_out
    );
    assert_eq!(
        curve_after_buy.real_sol_reserves,
        curve_before.real_sol_reserves + net_sol
    );
    assert_eq!(
        curve_after_buy.claimable_creator_fees,
        curve_before.claimable_creator_fees + creator_fee
    );
    assert!(!curve_after_buy.complete);

    let stake_vault = associated_token_address(
        stake_pool_pda,
        mint_pubkey,
        spl_token_2022::id().to_bytes().into(),
    );
    let stake_position_pda = Pubkey::find_program_address(
        &[
            nozz_state::StakePosition::SEED,
            mint_pubkey.as_ref(),
            payer.pubkey().as_ref(),
        ],
        &nozz_launchpad::ID,
    )
    .0;
    let stake_amount = 1_000_000;
    let initial_stake_vault_amount = program
        .rpc()
        .get_token_account_balance(&stake_vault)
        .unwrap()
        .amount
        .parse::<u64>()
        .unwrap();

    program
        .request()
        .accounts(nozz_accounts::Stake {
            staker: payer.pubkey(),
            mint: mint_pubkey,
            stake_pool: stake_pool_pda,
            staker_ata: buyer_token_ata,
            stake_vault,
            stake_position: stake_position_pda,
            token_program: spl_token_2022::id().to_bytes().into(),
            associated_token_program: spl_associated_token_account::id().to_bytes().into(),
            system_program: system_program::ID,
        })
        .args(nozz_instructions::Stake {
            amount: stake_amount,
        })
        .send()
        .unwrap();

    let stake_pool: nozz_state::StakePool = program.account(stake_pool_pda).unwrap();
    let stake_position: nozz_state::StakePosition = program.account(stake_position_pda).unwrap();
    assert_eq!(stake_pool.total_staked, stake_amount);
    assert_eq!(stake_position.amount_staked, stake_amount);
    assert!(stake_position.is_subscribed);
    assert_eq!(
        program
            .rpc()
            .get_token_account_balance(&stake_vault)
            .unwrap()
            .amount
            .parse::<u64>()
            .unwrap(),
        initial_stake_vault_amount + stake_amount
    );

    assert!(!curve_after_buy.should_graduate());
}

#[test]
fn test_graduation_condition_accepts_threshold_or_token_sellout() {
    let mut curve = nozz_state::BondingCurve {
        mint: Pubkey::default(),
        creator: Pubkey::default(),
        virtual_sol_reserves: 0,
        virtual_token_reserves: 0,
        real_sol_reserves: 0,
        real_token_reserves: 1,
        total_supply: 0,
        bonding_curve_allocation: 0,
        graduation_sol_threshold: 50_000_000_000,
        complete: false,
        migrated: false,
        claimable_creator_fees: 0,
        created_at: 0,
        total_volume: 0,
        bump: 0,
        vault_bump: 0,
    };

    assert!(!curve.should_graduate());
    curve.real_sol_reserves = curve.graduation_sol_threshold;
    assert!(curve.should_graduate());

    curve.real_sol_reserves = 0;
    curve.real_token_reserves = 0;
    assert!(curve.should_graduate());
}
