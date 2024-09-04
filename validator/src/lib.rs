pub mod bank_validator;
pub mod error;
pub mod settings;

pub use {
    bank_validator::BankValidator,
    error::{Error, Result},
};

#[macro_use]
extern crate log;

pub trait SvmValidator {
    type Transaction: Clone;
    type TransactionCheckResult;

    fn get_batch_results<'a>(
        &self,
        transactions: std::borrow::Cow<'a, [Self::Transaction]>,
    ) -> Vec<Self::TransactionCheckResult>;
}

pub trait TransactionChecks {
    type Transaction: Clone;
    type Error;

    fn transactions_sanity_check(
        &self,
        txs: &[Self::Transaction],
    ) -> std::result::Result<(), Self::Error>;

    fn transactions_conflict_check(
        &self,
        txs: &[Self::Transaction],
    ) -> std::result::Result<(), Self::Error>;
}
