use super::Processor;
use crate::Result;
use igloo_validator::{settings::Settings, BankValidator, SvmValidator};
use solana_runtime::bank::Bank;
use solana_sdk::transaction::SanitizedTransaction;
use solana_svm::transaction_processor::{
    LoadAndExecuteSanitizedTransactionsOutput, TransactionProcessingConfig,
    TransactionProcessingEnvironment,
};
use std::{borrow::Cow, sync::Arc};

pub struct BankProcessor {
    bank: Arc<Bank>,
    settings: Settings,
}

impl Processor for BankProcessor {
    fn process<'a>(
        &self,
        transactions: Cow<'a, [SanitizedTransaction]>,
    ) -> Result<LoadAndExecuteSanitizedTransactionsOutput> {
        let validator = BankValidator::new(self.bank.clone(), self.settings.clone());
        let result = validator.get_batch_results(transactions.clone());

        // use the bank's transaction processor to process the transactions
        let transaction_processor = self.bank.get_transaction_processor();
        let output = transaction_processor.load_and_execute_sanitized_transactions(
            self.bank.as_ref(),
            &transactions,
            result,
            &self.environment(),
            &self.processing_config(),
        );

        Ok(output)
    }
}

impl BankProcessor {
    pub fn new(bank: Arc<Bank>, settings: Settings) -> Self {
        Self { bank, settings }
    }

    fn environment(&self) -> TransactionProcessingEnvironment {
        let (blockhash, lamports_per_signature) =
            self.bank.last_blockhash_and_lamports_per_signature();
        let processing_environment = TransactionProcessingEnvironment {
            blockhash,
            epoch_total_stake: self.bank.epoch_total_stake(self.bank.epoch()),
            epoch_vote_accounts: self.bank.epoch_vote_accounts(self.bank.epoch()),
            feature_set: Arc::clone(&self.bank.feature_set),
            fee_structure: Some(self.bank.fee_structure()),
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
