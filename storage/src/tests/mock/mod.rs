use solana_sdk::{account::AccountSharedData, system_program};

pub mod processor;
pub mod txs;

pub fn system_account(lamports: u64) -> AccountSharedData {
    AccountSharedData::new(lamports, 0, &system_program::id())
}
