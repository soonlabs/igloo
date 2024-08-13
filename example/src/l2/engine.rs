use anyhow::Result;
use std::sync::Arc;

use rollups_interface::l2::{pool::TransactionPool, Block, Engine, EngineApi, Producer};

use crate::l1::attribute::PayloadAttributeImpl;

use super::{
    block::{BlockImpl, BlockPayloadImpl},
    head::L2HeadImpl,
    ledger::SharedLedger,
    pool::TransactionPoolImpl,
    producer::SvmProducer,
    L2Height,
};

pub struct SvmEngine {
    pool: TransactionPoolImpl,
    producer: SvmProducer,
    ledger: SharedLedger,
}

impl SvmEngine {
    pub fn new() -> Self {
        let ledger = SharedLedger::default();
        Self {
            pool: Default::default(),
            producer: SvmProducer::new(ledger.clone()),
            ledger,
        }
    }

    pub async fn produce_block(
        &mut self,
        attribute: PayloadAttributeImpl,
    ) -> anyhow::Result<BlockPayloadImpl> {
        let mut transactions = (*attribute.transactions).clone();
        transactions.extend(self.pool.next_batch(Default::default()));

        let new_attribute = PayloadAttributeImpl {
            transactions: Arc::new(transactions),
            epoch: attribute.epoch,
        };
        let block = self.producer.produce(new_attribute).await?;
        Ok(block)
    }
}

impl Engine for SvmEngine {
    type TransactionPool = TransactionPoolImpl;

    type Payload = BlockPayloadImpl;

    type Head = L2HeadImpl;

    type Block = BlockImpl;

    type BlockHeight = L2Height;

    fn pool(&self) -> &Self::TransactionPool {
        &self.pool
    }

    fn pool_mut(&mut self) -> &mut Self::TransactionPool {
        &mut self.pool
    }

    async fn get_head(&mut self, height: Self::BlockHeight) -> Result<Option<Self::Head>> {
        Ok(self
            .ledger
            .read()
            .await
            .blocks
            .get(&height)
            .map(|b| b.head().clone()))
    }
}

impl EngineApi<BlockImpl, L2HeadImpl> for SvmEngine {
    type Error = anyhow::Error;

    async fn new_block(&mut self, block: BlockImpl) -> Result<L2HeadImpl> {
        let head = self.ledger.write().await.new_block(block);
        Ok(head)
    }

    async fn reorg(&mut self, _reset_to: L2HeadImpl) -> Result<()> {
        todo!()
    }

    async fn finalize(&mut self, _block: L2HeadImpl) -> Result<()> {
        todo!()
    }
}
