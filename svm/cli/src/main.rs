use anyhow::Result;
use clap::Parser;
use igloo_interface::l2::{
    bank::{BankInfo, BankOperations},
    executor::{Config, Init},
};
use solana_sdk::{account::AccountSharedData, clock::Slot, hash::Hash, pubkey::Pubkey};
use solana_svm::{
    transaction_processing_callback::TransactionProcessingCallback,
    transaction_results::TransactionExecutionResult,
};
use svm_executor::{
    bank::{BankWrapper, WrapperConfig},
    mock::bank::{MockBankCallback, MockConfig},
    prelude::SimpleBuilder,
};

mod cli;

#[macro_use]
extern crate log;

fn main() -> Result<()> {
    env_logger::init();

    let cli = cli::Cli::parse();
    if cli.memory_mode {
        info!("use memory mode");
        run::<MockBankCallback, _>(cli, &MockConfig::default())
    } else {
        info!("use bank mode");
        run::<BankWrapper, _>(cli, &WrapperConfig::default())
    }
}

fn run<
    B: TransactionProcessingCallback
        + BankOperations<Pubkey = Pubkey, AccountSharedData = AccountSharedData>
        + BankInfo<Pubkey = Pubkey, Hash = Hash, Slot = Slot>
        + Init<Config = C>,
    C: Config,
>(
    cli: cli::Cli,
    cfg: &C,
) -> Result<()> {
    let mut builder = SimpleBuilder::<B>::init(cfg)?;
    for (pubkey, lamports, is_signer, is_writable) in cli.parse_accounts().unwrap() {
        builder.account_with_balance(pubkey, lamports, is_signer, is_writable);
    }
    if !cli.calldata.is_empty() {
        builder.calldata(decode_hex_with_prefix(&cli.calldata)?);
    }
    builder
        .program_path(cli.program_path)
        .program_buffer(cli.program_buffer)
        .v0_message(cli.enable_v0_message);

    let result = builder.build()?;

    for result in result.execution_results {
        match result {
            TransactionExecutionResult::Executed { details, .. } => {
                info!(
                    "Transaction executed\n\tStatus: {:?}\n\tLogs: {:?}\n\tReturns: {:?}",
                    details.status, details.log_messages, details.return_data
                );
            }
            TransactionExecutionResult::NotExecuted(e) => {
                error!("Transaction not executed, reason: {:?}", e);
            }
        }
    }

    Ok(())
}

fn decode_hex_with_prefix(s: &str) -> Result<Vec<u8>> {
    let trimmed = if let Some(stripped) = s.strip_prefix("0x") {
        stripped
    } else {
        s
    };
    Ok(hex::decode(trimmed)?)
}
