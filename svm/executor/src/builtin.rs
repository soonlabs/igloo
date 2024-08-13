use crate::{env::DEPLOYMENT_SLOT, mock::fork_graph::MockForkGraph};
use solana_program_runtime::loaded_programs::ProgramCacheEntry;
use solana_sdk::bpf_loader_upgradeable;
use solana_svm::{
    transaction_processing_callback::TransactionProcessingCallback,
    transaction_processor::TransactionBatchProcessor,
};

const BPF_LOADER_NAME: &str = "solana_bpf_loader_upgradeable_program";
const SYSTEM_PROGRAM_NAME: &str = "system_program";

pub fn register_builtins<CB: TransactionProcessingCallback>(
    mock_bank: &CB,
    batch_processor: &TransactionBatchProcessor<MockForkGraph>,
) {
    // We must register the bpf loader account as a loadable account, otherwise programs
    // won't execute.
    batch_processor.add_builtin(
        mock_bank,
        bpf_loader_upgradeable::id(),
        BPF_LOADER_NAME,
        ProgramCacheEntry::new_builtin(
            DEPLOYMENT_SLOT,
            BPF_LOADER_NAME.len(),
            solana_bpf_loader_program::Entrypoint::vm,
        ),
    );

    // In order to perform a transference of native tokens using the system instruction,
    // the system program builtin must be registered.
    batch_processor.add_builtin(
        mock_bank,
        solana_system_program::id(),
        SYSTEM_PROGRAM_NAME,
        ProgramCacheEntry::new_builtin(
            DEPLOYMENT_SLOT,
            SYSTEM_PROGRAM_NAME.len(),
            solana_system_program::system_processor::Entrypoint::vm,
        ),
    );
}
