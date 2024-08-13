use solana_sdk::{account::AccountSharedData, hash::Hash, pubkey::Pubkey};

mod wrapper;

pub use wrapper::BankWrapper;

pub trait BankOperations {
    fn insert_account(&mut self, key: Pubkey, data: AccountSharedData);

    fn deploy_program(&mut self, buffer: Vec<u8>) -> Pubkey;

    fn set_clock(&mut self);
}

pub trait BankInfo {
    fn last_blockhash(&self) -> Hash;

    fn execution_slot(&self) -> u64;

    fn execution_epoch(&self) -> u64;
}
