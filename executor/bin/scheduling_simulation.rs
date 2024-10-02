use igloo_executor::processor::TransactionProcessor;
use igloo_storage::blockstore::txs::CommitBatch;
use igloo_storage::execution::TransactionsResultWrapper;
use igloo_storage::{config::GlobalConfig, RollupStorage};
use igloo_verifier::settings::{Settings, Switchs};
use solana_program::hash::Hash;
use solana_sdk::account::AccountSharedData;
use solana_sdk::transaction::SanitizedTransaction;
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, system_transaction,
};
use std::borrow::Cow;
use std::error::Error;
use std::vec;

// Number of accounts and transactions to create
const NUM_ACCOUNTS: usize = 1;
// Amount to fund rich accounts with 100 SOL
const RICH_ACCOUNT_BALANCE: u64 = 100_000_000_000;

type E = Box<dyn Error>;

/// Generate a mocked transfer transaction from one account to another
///
/// # Parameters
/// * `from` - The `Keypair` of the sender account
/// * `to` - The `Pubkey` of the recipient account
/// * `amount` - The amount of lamports to transfer
///
/// # Returns
/// A `Result` containing a `SanitizedTransaction` representing the transfer, or an error
fn mocking_transfer_tx(
    from: &Keypair,
    to: &Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<SanitizedTransaction, E> {
    let transaction = system_transaction::transfer(from, to, amount, recent_blockhash);
    Ok(SanitizedTransaction::from_transaction_for_tests(
        transaction,
    ))
}

fn main() -> Result<(), E> {
    let rich_accounts: Vec<_> = (0..NUM_ACCOUNTS)
        .map(|_| (Keypair::new(), RICH_ACCOUNT_BALANCE))
        .collect();
    let empty_accounts: Vec<_> = (0..NUM_ACCOUNTS).map(|_| (Keypair::new(), 0)).collect();

    // Initialize the storage
    let ledger_path = tempfile::tempdir()?.into_path();
    let mut config = GlobalConfig::new_temp(&ledger_path)?;
    config.allow_default_genesis = true;

    // insert stub accounts into genesis
    {
        for i in 0..NUM_ACCOUNTS {
            config.genesis.add_account(
                rich_accounts[i].0.pubkey(),
                AccountSharedData::new(rich_accounts[i].1, 0, &system_program::id()),
            );
            config.genesis.add_account(
                empty_accounts[i].0.pubkey(),
                AccountSharedData::new(empty_accounts[i].1, 0, &system_program::id()),
            );
        }
    }

    let mut store = RollupStorage::new(config)?;
    store.init()?;
    store.bump()?;

    let recent_hash = store.current_bank().last_blockhash();

    // transfer tx and execution.
    let transfer_txs = rich_accounts
        .iter()
        .zip(empty_accounts.iter())
        .map(|(from, to)| {
            mocking_transfer_tx(
                &from.0,
                &to.0.pubkey(),
                RICH_ACCOUNT_BALANCE / 2,
                recent_hash,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bank_processor = TransactionProcessor::new(
        store.current_bank(),
        Settings {
            max_age: Default::default(),
            switchs: Switchs {
                tx_sanity_check: false,
                txs_conflict_check: true,
            },
            fee_structure: Default::default(),
        },
    );

    let execute_result = bank_processor.process(Cow::Borrowed(&transfer_txs))?;
    let result = store.commit_block(
        vec![TransactionsResultWrapper {
            output: execute_result,
        }],
        vec![CommitBatch::new(transfer_txs.into())],
    )?;

    // Print final balances
    println!("Final account balances:");
    for i in 0..NUM_ACCOUNTS {
        println!(
            "rich[i] balance = {}",
            store.balance(&rich_accounts[i].0.pubkey())
        );
        println!(
            "empty[i] balance = {}",
            store.balance(&empty_accounts[i].0.pubkey())
        );
    }
    Ok(())
}
