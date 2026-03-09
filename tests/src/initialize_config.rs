use anchor_client::solana_sdk::signer::Signer;
use nozz_launchpad::state as nozz_state;

use crate::utils::{send_initialize_config, setup_environment, Environment, InitializeConfigResponse};

#[test]
fn test_initialize_config() {
    let Environment {
        client: _,
        program,
        payer,
        config_pda,
        mint_pubkey: _,
        bonding_curve_pda: _,
        bonding_curve_vault_pda: _,
        bonding_curve_ata: _,
    } = setup_environment();

    let InitializeConfigResponse { fee_recipient } = send_initialize_config();

    let nozz_config_account: nozz_state::NozzLaunchpadConfig = program.account(config_pda).unwrap();

    assert_eq!(nozz_config_account.authority, payer.pubkey());
    assert_eq!(nozz_config_account.fee_recipient, fee_recipient);
    assert_eq!(nozz_config_account.platform_fee_bps, 25);
    assert_eq!(nozz_config_account.streamer_fee_bps, 75);
    assert_eq!(nozz_config_account.bonding_curve_supply_pct, 40);
    assert_eq!(
        nozz_config_account.graduation_sol_threshold,
        50 * 1_000_000_000
    );
    assert_eq!(nozz_config_account.token_count, 0);
    assert_eq!(
        nozz_config_account.initial_token_supply,
        100_000_000 * (1000000)
    );
}
