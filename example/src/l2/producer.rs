use anyhow::Ok;
use rollups_interface::l2::Producer;

use crate::l1::attribute::PayloadAttributeImpl;

use super::{block::BlockPayloadImpl, head::L2HeadImpl, ledger::SharedLedger};

pub struct SvmProducer {
    ledger: SharedLedger,
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
            entries: vec![], // TODO: produce entries by svm
        };
        Ok(block)
    }
}

impl SvmProducer {
    pub fn new(ledger: SharedLedger) -> Self {
        Self { ledger }
    }
}
