use anchor_lang::prelude::*;

#[event]
pub struct TokenCreated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub bonding_curve: Pubkey,
    pub total_supply: u64,
    pub bonding_curve_allocation: u64,
    pub timestamp: i64,
}
