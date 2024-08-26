use solana_sdk::{
    account::{AccountSharedData, ReadableAccount},
    pubkey::Pubkey,
    system_program,
};
use solana_svm::account_loader::LoadedTransaction;

pub mod processor;
pub mod txs;

pub fn system_account(lamports: u64) -> AccountSharedData {
    AccountSharedData::new(lamports, 0, &system_program::id())
}

pub fn assert_result_balance(account: Pubkey, lamports: Option<u64>, tx: &LoadedTransaction) {
    match tx.accounts.iter().find(|key| key.0 == account) {
        Some(recipient_data) => assert_eq!(recipient_data.1.lamports(), lamports.unwrap()),
        None => assert!(lamports.is_none()),
    }
}
