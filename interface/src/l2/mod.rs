use crate::error::Result;
use crate::l1::PayloadAttribute;

pub mod pool;

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

pub trait Entry {}

pub trait BlockPayload<P: PayloadAttribute>: TryFrom<P> {
    type Entry: Entry;

    fn entries(&self) -> &[Self::Entry];
}

pub trait Engine: EngineApi<Self::Payload, Self::Attribute, Self::Head> {
    type TransactionPool: pool::TransactionPool;
    type Attribute: PayloadAttribute;
    type Payload: BlockPayload<Self::Attribute>;
    type Head: L2Head;
    type BlockHeight: Copy;

    fn pool(&self) -> &Self::TransactionPool;

    fn pool_mut(&mut self) -> &mut Self::TransactionPool;

    fn next_payload(&mut self) -> Result<Option<Self::Payload>>;

    fn get_head(&mut self, height: Self::Head) -> Result<Option<Self::Head>>;
}

pub trait EngineApi<P: BlockPayload<A>, A: PayloadAttribute, H: L2Head> {
    fn new_block(&mut self, payload: P) -> Result<H>;

    fn reorg(&mut self, reset_to: H) -> Result<()>;

    fn finalize(&mut self, block: H) -> Result<()>;
}
