use anyhow::Result;
use solana_sdk::signature::Keypair;
use std::{path::Path, sync::Arc};
use tokio::sync::{mpsc::Sender, RwLock};

use rollups_interface::l2::{pool::TransactionPool, Block, Engine, EngineApi, L2Head, Producer};

use crate::l1::attribute::PayloadAttributeImpl;

use super::{
    block::{BlockImpl, BlockPayloadImpl},
    blockstore::{SharedStore, SimpleStore},
    head::L2HeadImpl,
    ledger::SharedLedger,
    pool::{SharedPool, TransactionPoolImpl},
    producer::SvmProducer,
    L2Height,
};

pub struct SvmEngine {
    pool: SharedPool,
    producer: SvmProducer,
    ledger: SharedLedger,
    blockstore: SharedStore,
    attribute_sender: Sender<PayloadAttributeImpl>,
}

impl SvmEngine {
    pub fn new(
        base_path: &Path,
        attribute_sender: Sender<PayloadAttributeImpl>,
    ) -> anyhow::Result<Self> {
        let blockstore = Arc::new(RwLock::new(SimpleStore::new(
            &base_path.join("blockstore"),
        )?));

        let ledger = SharedLedger::default();
        Ok(Self {
            pool: Default::default(),
            producer: SvmProducer::new(ledger.clone()),
            ledger,
            blockstore,
            attribute_sender,
        })
    }

    pub async fn produce_block(
        &mut self,
        attribute: PayloadAttributeImpl,
    ) -> anyhow::Result<BlockPayloadImpl> {
        let mut transactions = (*attribute.transactions).clone();
        let extra_txs = { self.pool.write().await.next_batch(Default::default()) };
        trace!(
            "produce block with {} deposit txs, {} txs from pool",
            transactions.len(),
            extra_txs.len()
        );
        transactions.extend(extra_txs);

        let new_attribute = PayloadAttributeImpl {
            transactions: Arc::new(transactions),
            epoch: attribute.epoch,
            sequence_number: attribute.sequence_number,
        };
        let block = self.producer.produce(new_attribute.clone()).await?;

        if let Err(e) = self.attribute_sender.send(new_attribute).await {
            error!("Failed to send attribute: {}", e);
        }

        Ok(block)
    }
}

impl Engine for SvmEngine {
    type TransactionPool = TransactionPoolImpl;

    type Payload = BlockPayloadImpl;

    type Head = L2HeadImpl;

    type Block = BlockImpl;

    type BlockHeight = L2Height;

    fn pool(&self) -> &SharedPool {
        &self.pool
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
        let head = self.ledger.write().await.new_block(block.clone());
        let size = self.blockstore.write().await.write_entries(
            head.block_height(),
            10,
            Some(head.block_height().saturating_sub(1)),
            true,
            &Keypair::new(),
            block.entries,
        )?;
        debug!(
            "create block at height: {}, shred size: {}",
            head.block_height(),
            size
        );
        Ok(head)
    }

    async fn reorg(&mut self, _reset_to: L2HeadImpl) -> Result<()> {
        todo!()
    }

    async fn finalize(&mut self, _block: L2HeadImpl) -> Result<()> {
        todo!()
    }
}
