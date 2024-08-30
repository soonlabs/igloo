pub trait TransactionsResult {
    type SuccessIn;
    type SuccessOut;

    fn success_txs(&self, extras: &[Self::SuccessIn]) -> Vec<Self::SuccessOut>;
}

pub trait TransactionSet {
    type Transaction;

    fn transactions(&self) -> &[Self::Transaction];
}

pub trait StorageOperations {
    type Error: std::fmt::Display;
    type TxsResult: TransactionsResult;
    type TransactionSet<'a>: TransactionSet;

    async fn commit<'a>(
        &mut self,
        result: Self::TxsResult,
        origin: Self::TransactionSet<'a>,
    ) -> Result<(), Self::Error>;

    /// Force save the storage to disk.
    async fn force_save(&mut self) -> Result<(), Self::Error>;

    async fn close(self) -> Result<(), Self::Error>;
}
