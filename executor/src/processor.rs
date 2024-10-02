use crate::Result;
use igloo_verifier::settings::Settings;
use igloo_verifier::BankVerifier;
use solana_runtime::bank::Bank;
use solana_sdk::transaction::SanitizedTransaction;
use solana_svm::transaction_processor::{
    LoadAndExecuteSanitizedTransactionsOutput, TransactionProcessingConfig,
    TransactionProcessingEnvironment,
};
use std::{borrow::Cow, sync::Arc};

pub struct TransactionProcessor {
    bank: Arc<Bank>,
    settings: Settings,
}

impl TransactionProcessor {
    pub fn new(bank: Arc<Bank>, settings: Settings) -> Self {
        Self { bank, settings }
    }

    pub fn process(
        &self,
        transactions: Cow<[SanitizedTransaction]>,
    ) -> Result<LoadAndExecuteSanitizedTransactionsOutput> {
        // TODO: There are some bug in `BankVerifier`, we need to fix it in future
        let validator = BankVerifier::new(self.bank.clone(), self.settings.clone());
        let results = validator.get_batch_results(transactions.clone());

        // use the bank's transaction processor to process the transactions
        let transaction_processor = self.bank.get_transaction_processor();
        let output = transaction_processor.load_and_execute_sanitized_transactions(
            self.bank.as_ref(),
            &transactions,
            results,
            &self.environment(),
            &self.processing_config(),
        );

        Ok(output)
    }

    fn environment(&self) -> TransactionProcessingEnvironment {
        let (blockhash, lamports_per_signature) =
            self.bank.last_blockhash_and_lamports_per_signature();
        let processing_environment = TransactionProcessingEnvironment {
            blockhash,
            epoch_total_stake: self.bank.epoch_total_stake(self.bank.epoch()),
            epoch_vote_accounts: self.bank.epoch_vote_accounts(self.bank.epoch()),
            feature_set: Arc::clone(&self.bank.feature_set),
            fee_structure: Some(&self.settings.fee_structure),
            lamports_per_signature,
            rent_collector: Some(self.bank.rent_collector()),
        };
        processing_environment
    }

    fn processing_config(&self) -> TransactionProcessingConfig {
        TransactionProcessingConfig {
            account_overrides: None,
            check_program_modification_slot: self.bank.check_program_modification_slot(),
            compute_budget: self.bank.compute_budget(),
            limit_to_load_programs: false,
            transaction_account_lock_limit: Some(self.bank.get_transaction_account_lock_limit()),
            ..Default::default()
        }
    }
}
