use anyhow::{Ok, Result};
use igloo_interface::{
    l1::PayloadAttribute,
    l2::{executor::Init, Entry, Producer},
};
use solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction};
use solana_svm::transaction_processor::TransactionBatchProcessor;
use std::{
    path::Path,
    sync::{Arc, RwLock},
};
use svm_executor::{
    bank::BankWrapper,
    builder::simple::create_transaction_processor,
    mock::fork_graph::{self, MockForkGraph},
    prelude::SimpleBuilder,
};

use crate::l1::attribute::PayloadAttributeImpl;

use super::{
    block::{BlockPayloadImpl, SimpleEntry},
    head::L2HeadImpl,
    ledger::SharedLedger,
    tx::L2Transaction,
};

pub struct SvmProducer {
    ledger: SharedLedger,
    bank: BankWrapper,
    tx_processor: Arc<TransactionBatchProcessor<MockForkGraph>>,
    fork_graph: Arc<RwLock<MockForkGraph>>,
    system_account: Pubkey,

    txs_per_entry: usize,
}

impl Producer for SvmProducer {
    type Attribute = PayloadAttributeImpl;

    type BlockPayload = BlockPayloadImpl;

    type Error = anyhow::Error;

    async fn produce(&self, attribute: Self::Attribute) -> anyhow::Result<Self::BlockPayload> {
        let new_height = { self.ledger.read().await.latest_height() + 1 };
        let block = BlockPayloadImpl {
            head: L2HeadImpl {
                hash: Default::default(),
                height: new_height,
                timestamp: chrono::Utc::now().timestamp() as u64,
            },
            entries: self.process_txs(attribute).await?,
        };
        Ok(block)
    }
}

impl SvmProducer {
    pub fn new(base_path: &Path, ledger: SharedLedger) -> anyhow::Result<Self> {
        let mut bank = BankWrapper::new_with_path(base_path, 4, &Default::default())?;
        let fork_graph = Arc::new(RwLock::new(fork_graph::MockForkGraph::default()));
        let tx_processor = Arc::new(create_transaction_processor(&mut bank, fork_graph.clone())?);
        let system_account = Pubkey::from([0u8; 32]);
        Ok(Self {
            ledger,
            bank,
            fork_graph,
            tx_processor,
            system_account,
            txs_per_entry: 64,
        })
    }

    async fn process_txs(&self, attribute: PayloadAttributeImpl) -> Result<Vec<SimpleEntry>> {
        // TODO: increase blockheight after processing
        let mut result = vec![];

        let mut txs = vec![];
        for tx in attribute.transactions().iter() {
            txs.push(self.process_single_tx(tx).await?);

            if txs.len() >= self.txs_per_entry {
                result.push(SimpleEntry::new(txs));
                txs = vec![];
            }
        }
        if !txs.is_empty() {
            result.push(SimpleEntry::new(txs));
        }

        debug!(
            "{} txs total, {} entries {} txs processed",
            attribute.transactions().len(),
            result.len(),
            result.iter().map(|e| e.tx_count()).sum::<usize>()
        );

        Ok(result)
    }

    // TODO: process batch transactions
    async fn process_single_tx(&self, tx: &L2Transaction) -> Result<VersionedTransaction> {
        const INIT_LAMPORTS: u64 = 900000;
        let mut builder = SimpleBuilder::<BankWrapper>::init(&Default::default())?;
        let path = self.get_program_path();
        let (result, txs) = builder
            .tx_processor(self.tx_processor.clone())
            .fork_graph(self.fork_graph.clone())
            .bank(self.bank.clone())
            .program_path(Some(path))
            .account_with_balance(tx.from, Some(INIT_LAMPORTS), true, true)
            .account_with_balance(tx.to, Some(INIT_LAMPORTS), false, true)
            .account_with_balance(self.system_account, None, false, false)
            .v0_message(true)
            .calldata(tx.amount.to_be_bytes().to_vec())
            .build_ex()?;

        if result
            .execution_results
            .first()
            .ok_or(anyhow::anyhow!("no result"))?
            .details()
            .ok_or(anyhow::anyhow!("no details"))?
            .status
            .is_err()
        {
            // simulate failed tx handler
            return Err(anyhow::anyhow!("tx failed"));
        }

        Ok(txs)
    }

    fn get_program_path(&self) -> String {
        "svm/executor/tests/simple_transfer_program.so".to_string()
    }
}
