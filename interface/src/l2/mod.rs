use std::sync::Arc;

use tokio::sync::RwLock;

use crate::l1::PayloadAttribute;

pub mod bank;
pub mod executor;
pub mod storage;
pub mod stream;

pub trait Transaction {
    type Address;
    type Amount: Copy;

    fn from(&self) -> &Self::Address;
    fn to(&self) -> &Self::Address;
    fn amount(&self) -> Self::Amount;
    fn calldata(&self) -> &[u8];
}

pub trait L2Head {
    type Hash;
    type BlockHeight;
    type Timestamp;

    fn block_hash(&self) -> Self::Hash;

    fn block_height(&self) -> Self::BlockHeight;

    fn timestamp(&self) -> Self::Timestamp;
}

pub trait Entry {
    fn tx_count(&self) -> usize;
}

pub trait Producer {
    type Attribute: PayloadAttribute;
    type BlockPayload: BlockPayload;
    type Error: std::fmt::Display;

    async fn produce(&self, attribute: Self::Attribute) -> Result<Self::BlockPayload, Self::Error>;
}

pub trait BlockPayload {
    type Entry: Entry;

    fn entries(&self) -> &[Self::Entry];
}

pub trait Block {
    type Head: L2Head;

    fn head(&self) -> &Self::Head;
}

pub trait Engine: EngineApi<Self::Block, Self::Head> {
    type TransactionStream: stream::TransactionStream;
    type Payload: BlockPayload;
    type Head: L2Head;
    type Block: Block<Head = Self::Head>;
    type BlockHeight: Copy;

    fn stream(&self) -> &Arc<RwLock<Self::TransactionStream>>;

    async fn get_head(
        &mut self,
        height: Self::BlockHeight,
    ) -> Result<Option<Self::Head>, Self::Error>;
}

pub trait EngineApi<B: Block, H: L2Head> {
    type Error: std::fmt::Display;

    async fn new_block(&mut self, block: B) -> Result<H, Self::Error>;

    async fn reorg(&mut self, reset_to: H) -> Result<(), Self::Error>;

    async fn finalize(&mut self, block: H) -> Result<(), Self::Error>;
}
