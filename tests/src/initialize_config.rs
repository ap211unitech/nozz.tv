use std::str::FromStr;

use anchor_client::{
    anchor_lang::prelude::system_program,
    solana_sdk::{
        commitment_config::CommitmentConfig, pubkey::Pubkey, signature::read_keypair_file,
        signer::Signer,
    },
    Client, Cluster,
};
use nozz_launchpad::{
    accounts as nozz_accounts, instruction as nozz_instructions, state as nozz_state,
    state::NozzLaunchpadConfig, InitializeConfigParams,
};

#[test]
fn test_initialize() {
    let program_id = "6zp1FgL5FShDjJoh8hoBztncuYWvSANvPHUZFceiVFsy";
    let anchor_wallet = std::env::var("ANCHOR_WALLET").unwrap();
    let payer = read_keypair_file(&anchor_wallet).unwrap();
    let fee_recipient = Pubkey::new_unique();

    let client = Client::new_with_options(Cluster::Localnet, &payer, CommitmentConfig::confirmed());
    let program_id = Pubkey::from_str(program_id).unwrap();
    let program = client.program(program_id).unwrap();

    // Derive config PDA
    let (config_pda, _) = Pubkey::find_program_address(&[NozzLaunchpadConfig::SEED], &program_id);

    let params = InitializeConfigParams {
        fee_recipient,
        platform_fee_bps: 25,
        streamer_fee_bps: 75,
        initial_token_supply: 1_000_000_000_000_000,
        graduation_sol_threshold: 50_000_000_000,
        bonding_curve_supply_pct: 40,
    };

    program
        .request()
        .accounts(nozz_accounts::InitializeConfig {
            authority: payer.pubkey(),
            nozz_launchpad_config: config_pda,
            system_program: system_program::ID,
        })
        .args(nozz_instructions::InitializeConfig { params })
        .send()
        .unwrap();

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
}
