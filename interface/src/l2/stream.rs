use super::Transaction;

pub trait BatchSettings {
    fn max_size(&self) -> usize;
}

pub trait TransactionStream {
    type TxIn: Transaction;
    type TxOut: Transaction;
    type Settings: BatchSettings;
    type Error: std::fmt::Display;

    async fn insert(&mut self, tx: Self::TxIn) -> Result<(), Self::Error>;

    async fn next_batch(&mut self, settings: Self::Settings) -> Vec<Self::TxOut>;
}
