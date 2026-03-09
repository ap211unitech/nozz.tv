use std::{rc::Rc, str::FromStr};

use anchor_client::{
    anchor_lang::{prelude::system_program, solana_program},
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::{read_keypair_file, Keypair},
        signer::Signer,
    },
    Client, Cluster, Program,
};

use nozz_launchpad::{
    accounts as nozz_accounts, instruction as nozz_instructions, BondingCurve, CreateTokenParams,
    InitializeConfigParams, NozzLaunchpadConfig,
};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022;

pub struct Environment {
    pub payer: Rc<Keypair>,
    pub program: Program<Rc<Keypair>>,
    pub client: Client<Rc<Keypair>>,
    pub config_pda: Pubkey,
    pub mint_pubkey: Pubkey,
    pub bonding_curve_pda: Pubkey,
    pub bonding_curve_vault_pda: Pubkey,
    pub bonding_curve_ata: Pubkey,
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

    // Derive config PDA
    let (config_pda, _) = Pubkey::find_program_address(&[NozzLaunchpadConfig::SEED], &program_id);

    let (mint_pubkey, _) = Pubkey::find_program_address(
        &[
            BondingCurve::CREATOR_TOKEN_MINT_SEED,
            payer.pubkey().as_ref(),
        ],
        &program_id,
    );

    let (bonding_curve_pda, _) =
        Pubkey::find_program_address(&[BondingCurve::SEED, mint_pubkey.as_ref()], &program_id);

    let (bonding_curve_vault_pda, _) = Pubkey::find_program_address(
        &[BondingCurve::VAULT_SEED, mint_pubkey.as_ref()],
        &program_id,
    );

    let bonding_curve_ata = get_associated_token_address_with_program_id(
        &bonding_curve_pda.to_bytes().into(),
        &mint_pubkey.to_bytes().into(),
        &spl_token_2022::id(),
    );

    Environment {
        payer,
        program,
        client,
        config_pda,
        mint_pubkey,
        bonding_curve_pda,
        bonding_curve_vault_pda,
        bonding_curve_ata: bonding_curve_ata.to_bytes().into(),
    }
}

pub struct InitializeConfigResponse {
    pub fee_recipient: Pubkey,
}

pub fn send_initialize_config() -> InitializeConfigResponse {
    let fee_recipient = Pubkey::new_unique();

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
        _ => InitializeConfigResponse { fee_recipient },
    }
}

pub fn send_create_token(params: CreateTokenParams) {
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

    program
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
        .args(nozz_instructions::CreateToken { params })
        .send()
        .unwrap();
}
