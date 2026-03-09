use anchor_client::{
    anchor_lang::{prelude::system_program, solana_program},
    solana_sdk::signer::Signer,
};
use nozz_launchpad::{
    accounts as nozz_accounts, instruction as nozz_instructions, state as nozz_state,
    CreateTokenParams,
};
use spl_token_2022::{
    extension::{BaseStateWithExtensions, StateWithExtensions},
    state::Mint,
};
use spl_token_metadata_interface::state::TokenMetadata;

use crate::utils::{send_create_token, send_initialize_config, setup_environment, Environment};

fn default_params() -> CreateTokenParams {
    CreateTokenParams {
        token_name: "NozzStream1".to_string(),
        token_ticker: "NST1".to_string(),
        token_uri: "https://raw.githubusercontent.com/solana-developers/opos-asset/main/assets/DeveloperPortal/metadata.json".to_string(),
    }
}

#[test]
fn test_create_token_success() {
    let Environment {
        client: _,
        program,
        payer,
        config_pda,
        mint_pubkey,
        bonding_curve_pda,
        bonding_curve_vault_pda: _,
        bonding_curve_ata,
    } = setup_environment();

    send_initialize_config();

    // Fetch config so we can compute expected values from it
    let config: nozz_state::NozzLaunchpadConfig = program.account(config_pda).unwrap();

    send_create_token(default_params());

    // Assert BondingCurve state
    let bc: nozz_state::BondingCurve = program.account(bonding_curve_pda).unwrap();

    let expected_bonding_allocation = config
        .initial_token_supply
        .checked_mul(config.bonding_curve_supply_pct as u64)
        .unwrap()
        .checked_div(100)
        .unwrap();

    assert_eq!(bc.mint, mint_pubkey);
    assert_eq!(bc.creator, payer.pubkey());
    assert_eq!(bc.total_supply, config.initial_token_supply);
    assert_eq!(bc.bonding_curve_allocation, expected_bonding_allocation);
    assert_eq!(bc.virtual_token_reserves, expected_bonding_allocation);
    assert_eq!(bc.real_token_reserves, expected_bonding_allocation);
    assert_eq!(
        bc.virtual_sol_reserves,
        nozz_launchpad::utils::VIRTUAL_SOL_SEED
    );
    assert_eq!(bc.real_sol_reserves, 0);
    assert_eq!(bc.pending_creator_fees, 0);
    assert_eq!(bc.total_volume, 0);
    assert_eq!(bc.graduation_sol_threshold, config.graduation_sol_threshold);
    assert!(!bc.complete);
    assert!(!bc.migrated);

    // Assert mint was created with Token-2022
    let mint_account = program.rpc().get_account(&mint_pubkey).unwrap();

    // Owner must be the Token-2022 program, not legacy spl-token
    assert_eq!(
        mint_account.owner.to_string(),
        spl_token_2022::id().to_string()
    );

    // Assert metadata extension was written into the mint
    // Unpack the Token-2022 mint account and read the embedded TokenMetadata
    let mint_data = StateWithExtensions::<Mint>::unpack(&mint_account.data).unwrap();
    let metadata = mint_data
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    let params = default_params();

    assert_eq!(metadata.name, params.token_name);
    assert_eq!(metadata.symbol, params.token_ticker);
    assert_eq!(metadata.uri, params.token_uri);
    // update_authority should be the bonding_curve PDA
    assert_eq!(
        metadata.update_authority.0,
        bonding_curve_pda.to_bytes().into()
    );
    // Assert bonding_curve ATA holds the full supply
    let ata_balance = program
        .rpc()
        .get_token_account_balance(&bonding_curve_ata.to_bytes().into())
        .unwrap();

    // ATA should hold total_supply (already has decimals applied from mint_to)
    assert_eq!(
        ata_balance.amount.parse::<u64>().unwrap(),
        config.initial_token_supply,
    );
}

#[test]
fn test_create_token_duplicate_fails() {
    let Environment {
        client: _,
        program,
        payer,
        config_pda,
        mint_pubkey,
        bonding_curve_pda,
        bonding_curve_vault_pda,
        bonding_curve_ata,
    } = setup_environment();

    send_initialize_config();
    send_create_token(default_params());

    // Should fail when trying to create a second token — mint PDA already exists
    let result = program
        .request()
        .accounts(nozz_accounts::CreateToken {
            creator: payer.pubkey(),
            nozz_launchpad_config: config_pda,
            mint: mint_pubkey,
            bonding_curve: bonding_curve_pda,
            bonding_curve_vault: bonding_curve_vault_pda,
            bonding_curve_ata: bonding_curve_ata.to_bytes().into(),
            token_program: spl_token_2022::id().to_bytes().into(),
            associated_token_program: spl_associated_token_account::id().to_bytes().into(),
            system_program: system_program::ID,
            rent: solana_program::sysvar::rent::ID,
        })
        .args(nozz_instructions::CreateToken {
            params: CreateTokenParams {
                token_name: "NozzStream2".to_string(),
                token_ticker: "NST2".to_string(),
                token_uri: "https://arweave.net/test-metadata-hash".to_string(),
            },
        })
        .send();

    assert!(
        result.is_err(),
        "Expected second token creation to fail but it succeeded"
    );

    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("already in use") || err_str.contains("custom program error: 0x0"),
        "Expected AccountAlreadyInitialized error, got: {}",
        err_str
    );
}
