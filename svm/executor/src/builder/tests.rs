use std::env;

use igloo_interface::l2::executor::Init;
use solana_sdk::{
    account::ReadableAccount, clock::Clock, pubkey::Pubkey, sysvar::SysvarId,
    transaction::TransactionError,
};
use solana_svm::{
    transaction_processing_callback::TransactionProcessingCallback,
    transaction_results::TransactionExecutionResult,
};

use crate::{mock::bank::MockBankCallback, prelude::SimpleBuilder};

fn get_program_path(name: &str) -> String {
    let mut dir = env::current_dir().unwrap();
    dir.push("tests");
    let name = name.replace('-', "_");
    dir.push(name + "_program.so");
    dir.to_str().unwrap().to_string()
}

#[test]
fn hello_program_works() {
    let path = get_program_path("hello-solana");

    let mut builder = SimpleBuilder::<MockBankCallback>::init(&Default::default()).unwrap();
    let result = builder
        .program_path(Some(path))
        .build()
        .expect("Failed to build transaction");

    assert_eq!(result.execution_results.len(), 1);
    assert!(result.execution_results[0]
        .details()
        .unwrap()
        .status
        .is_ok());
    let logs = result.execution_results[0]
        .details()
        .unwrap()
        .log_messages
        .as_ref()
        .unwrap();
    assert!(logs.contains(&"Program log: Hello, Solana!".to_string()));
}

#[test]
fn clock_sysvar_works() {
    let path = get_program_path("clock-sysvar");

    let mut builder = SimpleBuilder::<MockBankCallback>::init(&Default::default()).unwrap();
    let result = builder
        .program_path(Some(path))
        .build()
        .expect("Failed to build transaction");

    assert_eq!(result.execution_results.len(), 1);
    assert!(result.execution_results[0]
        .details()
        .unwrap()
        .status
        .is_ok());
    let return_data = result.execution_results[0]
        .details()
        .unwrap()
        .return_data
        .as_ref()
        .unwrap();
    let time = i64::from_be_bytes(return_data.data[0..8].try_into().unwrap());
    let clock_data = builder
        .get_bank()
        .get_account_shared_data(&Clock::id())
        .unwrap();
    let clock_info: Clock = bincode::deserialize(clock_data.data()).unwrap();
    assert_eq!(clock_info.unix_timestamp, time);
}

#[test]
fn simple_transfer_works() {
    let path = get_program_path("simple-transfer");
    let sender = Pubkey::new_unique();
    let recipient = Pubkey::new_unique();
    let system_account = Pubkey::from([0u8; 32]);
    println!("system_account: {}", system_account);

    let mut builder = SimpleBuilder::<MockBankCallback>::init(&Default::default()).unwrap();
    let result = builder
        .program_path(Some(path))
        .account_with_balance(sender, Some(900000), true, true)
        .account_with_balance(recipient, Some(900000), false, true)
        .account_with_balance(system_account, None, false, false)
        .calldata(vec![0, 0, 0, 0, 0, 0, 0, 10])
        .v0_message(true)
        .build()
        .expect("Failed to build transaction");

    assert_eq!(result.execution_results.len(), 1);
    assert!(result.execution_results[0]
        .details()
        .unwrap()
        .status
        .is_ok());
    let recipient_data = result.loaded_transactions[0]
        .as_ref()
        .unwrap()
        .accounts
        .iter()
        .find(|key| key.0 == recipient)
        .unwrap();
    assert_eq!(recipient_data.1.lamports(), 900010);
}

#[test]
fn simple_transfer_failed_with_insufficient_balance() {
    let path = get_program_path("simple-transfer");
    let sender = Pubkey::new_unique();
    let recipient = Pubkey::new_unique();
    let system_account = Pubkey::from([0u8; 32]);

    let mut builder = SimpleBuilder::<MockBankCallback>::init(&Default::default()).unwrap();
    let result = builder
        .program_path(Some(path))
        .account_with_balance(sender, Some(900000), true, true)
        .account_with_balance(recipient, Some(900000), false, true)
        .account_with_balance(system_account, None, false, false)
        .calldata(900050u64.to_be_bytes().to_vec())
        .v0_message(true)
        .build()
        .expect("Failed to build transaction");

    assert_eq!(result.execution_results.len(), 1);
    assert!(result.execution_results[0]
        .details()
        .unwrap()
        .status
        .is_err());
    assert!(result.execution_results[0]
        .details()
        .unwrap()
        .log_messages
        .as_ref()
        .unwrap()
        .contains(&"Transfer: insufficient lamports 900000, need 900050".to_string()));
}

#[test]
fn simple_transfer_failed_with_custom_check_error() {
    let path = get_program_path("simple-transfer");
    let sender = Pubkey::new_unique();
    let recipient = Pubkey::new_unique();
    let system_account = Pubkey::from([0u8; 32]);

    let mut builder = SimpleBuilder::<MockBankCallback>::init(&Default::default()).unwrap();
    let result = builder
        .program_path(Some(path))
        .account_with_balance(sender, Some(900000), true, true)
        .account_with_balance(recipient, Some(900000), false, true)
        .account_with_balance(system_account, None, false, false)
        .calldata(900050u64.to_be_bytes().to_vec())
        .v0_message(true)
        .check_result(Err(TransactionError::BlockhashNotFound))
        .build()
        .expect("Failed to build transaction");

    assert_eq!(result.execution_results.len(), 1);
    assert!(matches!(
        result.execution_results[0],
        TransactionExecutionResult::NotExecuted(TransactionError::BlockhashNotFound)
    ));
}
