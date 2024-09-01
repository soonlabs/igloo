use crate::Result;
use solana_sdk::transaction::SanitizedTransaction;
use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;
use std::borrow::Cow;

pub mod bank;

pub trait Processor {
    fn process<'a>(
        &self,
        transactions: Cow<'a, [SanitizedTransaction]>,
    ) -> Result<LoadAndExecuteSanitizedTransactionsOutput>;
}
