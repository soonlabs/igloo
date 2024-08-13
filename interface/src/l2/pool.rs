use super::Transaction;
use crate::error::Result;

pub trait BatchSettings {
    fn max_size(&self) -> usize;
}

pub trait TransactionPool {
    type TxIn: Transaction;
    type TxOut: Transaction;
    type Settings: BatchSettings;

    fn insert(&mut self, tx: Self::TxIn) -> Result<()>;

    fn next_batch(&mut self, settings: Self::Settings) -> Vec<Self::TxOut>;
}
