pub mod attribute;

pub use attribute::*;

pub trait DepositTransaction {
    type Address;
    type Amount: Copy;

    fn from(&self) -> &Self::Address;
    fn to(&self) -> &Self::Address;
    fn amount(&self) -> Self::Amount;
    fn calldata(&self) -> &[u8];
}

pub trait L1Head {
    type Hash;
    type BlockHeight;
    type Timestamp;

    fn block_hash(&self) -> Self::Hash;

    fn block_height(&self) -> Self::BlockHeight;

    fn timestamp(&self) -> Self::Timestamp;
}

pub trait BatchInfo {
    type Hash;
    fn root_hash(&self) -> Self::Hash;
}

pub trait L1BlockInfo<P: PayloadAttribute>: TryInto<P> {
    type DepositTx: DepositTransaction;
    type Batch: BatchInfo;
    type L1Head: L1Head;

    fn l1_head(&self) -> &Self::L1Head;

    fn deposit_transactions(&self) -> &[Self::DepositTx];

    fn batch_info(&self) -> Option<&Self::Batch>;
}
