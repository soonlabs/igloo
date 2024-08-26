use crate::RollupStorage;

use super::txs::{create_svm_transactions, MockTransaction};
use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_compute_budget::compute_budget::ComputeBudget;
use solana_program_runtime::loaded_programs::ProgramCacheEntry;
use solana_sdk::transaction::SanitizedTransaction;
use solana_svm::{
    account_loader::CheckedTransactionDetails,
    transaction_processing_callback::TransactionProcessingCallback,
    transaction_processor::{LoadAndExecuteSanitizedTransactionsOutput, TransactionBatchProcessor},
};
use solana_system_program::system_processor;
use svm_executor::mock::fork_graph::MockForkGraph;
use {
    solana_sdk::{
        feature_set::FeatureSet, fee::FeeStructure, hash::Hash, rent_collector::RentCollector,
    },
    solana_svm::transaction_processor::{
        TransactionProcessingConfig, TransactionProcessingEnvironment,
    },
    std::sync::{Arc, RwLock},
};

pub fn process_transfers(
    store: &RollupStorage,
    transactions: &[MockTransaction],
) -> LoadAndExecuteSanitizedTransactionsOutput {
    let svm_transactions = create_svm_transactions(transactions);
    process_transfers_ex(store, svm_transactions)
}

pub fn process_transfers_ex(
    store: &RollupStorage,
    svm_transactions: Vec<SanitizedTransaction>,
) -> LoadAndExecuteSanitizedTransactionsOutput {
    // PayTube default configs.
    //
    // These can be configurable for channel customization, including
    // imposing resource or feature restrictions, but more commonly they
    // would likely be hoisted from the cluster.
    let compute_budget = ComputeBudget::default();
    let feature_set = FeatureSet::all_enabled();
    let fee_structure = FeeStructure::default();
    let lamports_per_signature = fee_structure.lamports_per_signature;
    let rent_collector = RentCollector::default();

    // Solana SVM transaction batch processor.
    //
    // Creates an instance of `TransactionBatchProcessor`, which can be
    // used by PayTube to process transactions using the SVM.
    //
    // This allows programs such as the System and Token programs to be
    // translated and executed within a provisioned virtual machine, as
    // well as offers many of the same functionality as the lower-level
    // Solana runtime.
    let fork_graph = Arc::new(RwLock::new(MockForkGraph {}));
    let processor = create_transaction_batch_processor(
        store,
        &feature_set,
        &compute_budget,
        Arc::clone(&fork_graph),
    );

    // The PayTube transaction processing runtime environment.
    let processing_environment = TransactionProcessingEnvironment {
        blockhash: Hash::default(),
        epoch_total_stake: None,
        epoch_vote_accounts: None,
        feature_set: Arc::new(feature_set),
        fee_structure: Some(&fee_structure),
        lamports_per_signature,
        rent_collector: Some(&rent_collector),
    };

    // The PayTube transaction processing config for Solana SVM.
    let processing_config = TransactionProcessingConfig {
        compute_budget: Some(compute_budget),
        ..Default::default()
    };

    // Process transactions with the SVM API.
    processor.load_and_execute_sanitized_transactions(
        store,
        &svm_transactions,
        get_transaction_check_results(svm_transactions.len(), lamports_per_signature),
        &processing_environment,
        &processing_config,
    )
}

/// This function encapsulates some initial setup required to tweak the
/// `TransactionBatchProcessor` for use within MockTpu.
///
/// We're simply configuring the mocked fork graph on the SVM API's program
/// cache, then adding the System program to the processor's builtins.
fn create_transaction_batch_processor<CB: TransactionProcessingCallback>(
    callbacks: &CB,
    feature_set: &FeatureSet,
    compute_budget: &ComputeBudget,
    fork_graph: Arc<RwLock<MockForkGraph>>,
) -> TransactionBatchProcessor<MockForkGraph> {
    let processor = TransactionBatchProcessor::<MockForkGraph>::default();

    {
        let mut cache = processor.program_cache.write().unwrap();

        // Initialize the mocked fork graph.
        // let fork_graph = Arc::new(RwLock::new(MockTpuForkGraph {}));
        cache.fork_graph = Some(Arc::downgrade(&fork_graph));

        // Initialize a proper cache environment.
        // (Use Loader v4 program to initialize runtime v2 if desired)
        cache.environments.program_runtime_v1 = Arc::new(
            create_program_runtime_environment_v1(feature_set, compute_budget, false, false)
                .unwrap(),
        );
    }

    // Add the system program builtin.
    processor.add_builtin(
        callbacks,
        solana_system_program::id(),
        "system_program",
        ProgramCacheEntry::new_builtin(
            0,
            b"system_program".len(),
            system_processor::Entrypoint::vm,
        ),
    );

    processor
}

fn get_transaction_check_results(
    len: usize,
    lamports_per_signature: u64,
) -> Vec<solana_sdk::transaction::Result<CheckedTransactionDetails>> {
    vec![
        solana_sdk::transaction::Result::Ok(CheckedTransactionDetails {
            nonce: None,
            lamports_per_signature,
        });
        len
    ]
}
