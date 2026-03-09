use std::{rc::Rc, str::FromStr};

use anchor_client::{
    anchor_lang::prelude::system_program,
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::{read_keypair_file, Keypair},
        signer::Signer,
    },
    Client, Cluster, Program,
};

use nozz_launchpad::{
    accounts as nozz_accounts, instruction as nozz_instructions, InitializeConfigParams,
    NozzLaunchpadConfig,
};

pub struct Environment {
    pub payer: Rc<Keypair>,
    pub program: Program<Rc<Keypair>>,
    pub client: Client<Rc<Keypair>>,
}

pub fn setup_environment() -> Environment {
    let program_id = "5pAxXXdL7NzFKqpp6TnuxBojeFuKEijX6amRvY4G8dvA";
    let anchor_wallet = std::env::var("ANCHOR_WALLET").unwrap();

    let payer = Rc::new(read_keypair_file(anchor_wallet).unwrap());

    let client = Client::new_with_options(
        Cluster::Localnet,
        payer.clone(),
        CommitmentConfig::confirmed(),
    );

    let program_id = Pubkey::from_str(program_id).unwrap();
    let program = client.program(program_id).unwrap();

    Environment {
        payer,
        program,
        client,
    }
}

pub struct InitializeConfigResponse {
    pub fee_recipient: Pubkey,
    pub config_pda: Pubkey,
}

pub fn initialize_config() -> InitializeConfigResponse {
    let fee_recipient = Pubkey::new_unique();

    let Environment {
        client: _,
        program,
        payer,
    } = setup_environment();
    let program_id = program.id();

    // Derive config PDA
    let (config_pda, _) = Pubkey::find_program_address(&[NozzLaunchpadConfig::SEED], &program_id);

    let params = InitializeConfigParams {
        fee_recipient,
        platform_fee_bps: 25,
        streamer_fee_bps: 75,
        initial_token_supply: 100_000_000, // without decimals
        graduation_sol_threshold: 50_000_000_000,
        bonding_curve_supply_pct: 40,
    };

    let response = program
        .request()
        .accounts(nozz_accounts::InitializeConfig {
            authority: payer.pubkey(),
            nozz_launchpad_config: config_pda,
            system_program: system_program::ID,
        })
        .args(nozz_instructions::InitializeConfig { params })
        .send();

    // Either config initialize or fail (if already initialize), always return response
    match response {
        _ => InitializeConfigResponse {
            fee_recipient,
            config_pda,
        },
    }
}
