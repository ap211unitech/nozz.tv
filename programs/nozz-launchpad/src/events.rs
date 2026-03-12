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

#[event]
pub struct TradeEvent {
    pub mint: Pubkey,
    pub trader: Pubkey,
    pub is_buy: bool,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub platform_fee: u64,
    pub creator_fee: u64,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub timestamp: i64,
}

#[event]
pub struct FeesClaimed {
    pub bonding_curve: Pubkey,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}
