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

/***************  Staking events ***************/

#[event]
pub struct Staked {
    pub mint: Pubkey,
    pub staker: Pubkey,
    pub amount: u64,
    pub total_staked: u64,      // user's total after this stake
    pub pool_total_staked: u64, // pool total after this stake
    pub is_subscribed: bool,
    pub timestamp: i64,
}

#[event]
pub struct Unstaked {
    pub mint: Pubkey,
    pub staker: Pubkey,
    pub amount: u64,
    pub total_staked: u64, // user's remaining after unstake
    pub pool_total_staked: u64,
    pub is_subscribed: bool,
    pub timestamp: i64,
}

#[event]
pub struct MinStakeUpdated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub old_min_stake: u64,
    pub new_min_stake: u64,
    pub timestamp: i64,
}

#[event]
pub struct RewardsClaimed {
    pub mint: Pubkey,
    pub staker: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}
