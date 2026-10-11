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

// Build the canonical Raydium graduation account set for a given token.
// The helper keeps the test aligned with the actual program assumptions: the token/WSOL
// pool ordering, the derived PDA addresses, and the expected LP-token custody accounts.
fn graduate_to_dex_accounts(
    caller: Pubkey,
    creator: Pubkey,
    bonding_curve: Pubkey,
    mint: Pubkey,
    bonding_curve_sol_vault: Pubkey,
    bonding_curve_token_vault: Pubkey,
) -> (nozz_accounts::GraduateToDex, Pubkey, Pubkey) {
    let cp_swap_program = raydium_cp_swap::ID;
    let wsol_mint = spl_token::native_mint::ID;
    let (token_0_mint, token_1_mint) = if mint < wsol_mint {
        (mint, wsol_mint)
    } else {
        (wsol_mint, mint)
    };
    let amm_config = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::AMM_CONFIG_SEED.as_bytes(),
            &0u16.to_be_bytes(),
        ],
        &cp_swap_program,
    )
    .0;
    let pool = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::POOL_SEED.as_bytes(),
            amm_config.as_ref(),
            token_0_mint.as_ref(),
            token_1_mint.as_ref(),
        ],
        &cp_swap_program,
    )
    .0;
    let lp_mint = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::POOL_LP_MINT_SEED.as_bytes(),
            pool.as_ref(),
        ],
        &cp_swap_program,
    )
    .0;
    let token_0_vault = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::POOL_VAULT_SEED.as_bytes(),
            pool.as_ref(),
            token_0_mint.as_ref(),
        ],
        &cp_swap_program,
    )
    .0;
    let token_1_vault = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::POOL_VAULT_SEED.as_bytes(),
            pool.as_ref(),
            token_1_mint.as_ref(),
        ],
        &cp_swap_program,
    )
    .0;
    let observation_state = Pubkey::find_program_address(
        &[
            raydium_cp_swap::states::OBSERVATION_SEED.as_bytes(),
            pool.as_ref(),
        ],
        &cp_swap_program,
    )
    .0;

    let locked_lp_token =
        associated_token_address(bonding_curve, lp_mint, spl_token::ID.to_bytes().into());
    let accounts = nozz_accounts::GraduateToDex {
        caller,
        creator,
        bonding_curve,
        mint,
        wsol_mint,
        bonding_curve_sol_vault,
        bonding_curve_token_vault,
        caller_token_ata: associated_token_address(
            caller,
            mint,
            spl_token_2022::id().to_bytes().into(),
        ),
        caller_wsol_ata: associated_token_address(
            caller,
            wsol_mint,
            spl_token::ID.to_bytes().into(),
        ),
        create_pool_fee: raydium_cp_swap::create_pool_fee_reveiver::ID,
        amm_config,
        raydium_authority: Pubkey::find_program_address(
            &[raydium_cp_swap::AUTH_SEED.as_bytes()],
            &cp_swap_program,
        )
        .0,
        raydium_pool_state: pool,
        raydium_lp_mint: lp_mint,
        caller_lp_token: associated_token_address(caller, lp_mint, spl_token::ID.to_bytes().into()),
        locked_lp_token,
        token_0_vault,
        token_1_vault,
        observation_state,
        token_program: spl_token::ID.to_bytes().into(),
        token_2022_program: spl_token_2022::id().to_bytes().into(),
        associated_token_program: spl_associated_token_account::id().to_bytes().into(),
        system_program: system_program::ID,
        rent: anchor_client::solana_sdk::sysvar::rent::ID,
        raydium_cp_swap_program: cp_swap_program,
    };
    (accounts, locked_lp_token, pool)
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
// End-to-end trading flow: create a token, buy into the curve, stake for subscriber status,
// then graduate the curve into a real DEX pool once the graduation threshold is met.
fn test_buy_stake_and_graduate_to_dex() {
    let Environment {
        program,
        payer,
        config_pda,
        ..
    } = setup_environment();

    send_initialize_config();

    // This test creates a fresh creator token and then exercises the core on-chain lifecycle:
    // buy -> accumulate fees -> stake/reward state -> graduation to a real liquidity pool.
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

    // The first trade establishes the curve's initial reserve state and confirms the
    // buyer receives tokens while fees are split between treasury and creator vault.
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

    // Staking is validated by checking both the pool accounting and the actual token vault
    // balance after the staker deposits their creator tokens into the staking pool.
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

    // We explicitly reject graduation before the bonding curve is actually complete, which
    // ensures the program enforces the lifecycle gate and prevents premature migration.
    let (graduation_accounts, locked_lp_token, pool_state) = graduate_to_dex_accounts(
        payer.pubkey(),
        creator.pubkey(),
        bonding_curve_pda,
        mint_pubkey,
        bonding_curve_vault_pda,
        bonding_curve_ata,
    );
    let pool_vaults = [
        graduation_accounts.token_0_vault,
        graduation_accounts.token_1_vault,
    ];
    let graduation_result = program
        .request()
        .accounts(graduation_accounts)
        .args(nozz_instructions::GraduateToDex {})
        .send();

    let graduation_error = graduation_result.unwrap_err().to_string();
    assert!(
        graduation_error.contains("BondingCurveNotComplete") || graduation_error.contains("0x1778"),
        "Expected BondingCurveNotComplete error, got: {graduation_error}"
    );

    let curve_after_rejected_graduation: nozz_state::BondingCurve =
        program.account(bonding_curve_pda).unwrap();
    assert!(!curve_after_rejected_graduation.complete);
    assert!(!curve_after_rejected_graduation.migrated);

    let fees_bps = u64::from(config.platform_fee_bps) + u64::from(config.streamer_fee_bps);
    let remaining_sol = curve_after_rejected_graduation
        .graduation_sol_threshold
        .checked_sub(curve_after_rejected_graduation.real_sol_reserves)
        .unwrap();
    let graduation_buy_amount = remaining_sol
        .checked_mul(10_000)
        .unwrap()
        .div_ceil(10_000 - fees_bps)
        + 2;
    let net_sol = graduation_buy_amount
        - utils::calculate_fee(graduation_buy_amount, config.platform_fee_bps).unwrap()
        - utils::calculate_fee(graduation_buy_amount, config.streamer_fee_bps).unwrap();
    let tokens_out = utils::get_tokens_for_sol(
        curve_after_rejected_graduation.virtual_sol_reserves,
        curve_after_rejected_graduation.virtual_token_reserves,
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
            sol_amount: graduation_buy_amount,
            min_tokens_out: tokens_out,
        })
        .send()
        .unwrap();

    // Once the real reserve reaches the configured graduation threshold, the curve is marked
    // complete and the migration path becomes valid.
    let curve_before_graduation: nozz_state::BondingCurve =
        program.account(bonding_curve_pda).unwrap();
    assert!(curve_before_graduation.complete);
    let creator_balance_before_graduation = program.rpc().get_balance(&creator.pubkey()).unwrap();

    program
        .request()
        .instruction(
            anchor_client::solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(
                1_000_000,
            ),
        )
        .accounts(
            graduate_to_dex_accounts(
                payer.pubkey(),
                creator.pubkey(),
                bonding_curve_pda,
                mint_pubkey,
                bonding_curve_vault_pda,
                bonding_curve_ata,
            )
            .0,
        )
        .args(nozz_instructions::GraduateToDex {})
        .send()
        .unwrap();

    let curve_after_graduation: nozz_state::BondingCurve =
        program.account(bonding_curve_pda).unwrap();
    assert!(curve_after_graduation.migrated);
    assert_eq!(curve_after_graduation.real_sol_reserves, 0);
    assert_eq!(curve_after_graduation.real_token_reserves, 0);
    assert_eq!(curve_after_graduation.claimable_creator_fees, 0);
    assert_eq!(
        program.rpc().get_balance(&creator.pubkey()).unwrap(),
        creator_balance_before_graduation + curve_before_graduation.claimable_creator_fees
    );
    let pool_account = program.rpc().get_account(&pool_state).unwrap();
    assert_eq!(pool_account.owner, raydium_cp_swap::ID);
    for vault in pool_vaults {
        let balance = program
            .rpc()
            .get_token_account_balance(&vault)
            .unwrap()
            .amount
            .parse::<u64>()
            .unwrap();
        assert!(
            balance > 0,
            "Expected both DEX pool vaults to receive liquidity"
        );
    }
    let locked_lp_balance = program
        .rpc()
        .get_token_account_balance(&locked_lp_token)
        .unwrap()
        .amount
        .parse::<u64>()
        .unwrap();
    assert!(locked_lp_balance > 0);

    let second_graduation = program
        .request()
        .accounts(
            graduate_to_dex_accounts(
                payer.pubkey(),
                creator.pubkey(),
                bonding_curve_pda,
                mint_pubkey,
                bonding_curve_vault_pda,
                bonding_curve_ata,
            )
            .0,
        )
        .args(nozz_instructions::GraduateToDex {})
        .send();
    let second_graduation_error = second_graduation.unwrap_err().to_string();
    assert!(
        second_graduation_error.contains("AlreadyGraduated")
            || second_graduation_error.contains("0x1779"),
        "Expected AlreadyGraduated error, got: {second_graduation_error}"
    );
}

#[test]
fn test_graduate_condition_accepts_threshold_or_token_sellout() {
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

    curve.real_sol_reserves = curve.graduation_sol_threshold - 1;
    curve.real_token_reserves = 0;
    assert!(curve.should_graduate());

    curve.real_sol_reserves = curve.graduation_sol_threshold - 1;
    curve.real_token_reserves = 1;
    assert!(!curve.should_graduate());
}
