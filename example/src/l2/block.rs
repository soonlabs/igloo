use igloo_interface::l2::{Block, BlockPayload, Entry};
use solana_sdk::transaction::VersionedTransaction;

use super::head::L2HeadImpl;

#[derive(Clone)]
pub struct SimpleEntry {
    pub inner: solana_entry::entry::Entry,
}

impl SimpleEntry {
    pub fn new(txs: Vec<VersionedTransaction>) -> Self {
        Self {
            inner: solana_entry::entry::Entry {
                num_hashes: txs.len() as u64,
                hash: Default::default(),
                transactions: txs,
            },
        }
    }
}

#[derive(Default)]
pub struct BlockPayloadImpl {
    pub head: L2HeadImpl,
    pub entries: Vec<SimpleEntry>,
}

impl BlockPayload for BlockPayloadImpl {
    type Entry = SimpleEntry;

    fn entries(&self) -> &[Self::Entry] {
        &self.entries
    }
}

impl Entry for SimpleEntry {
    fn tx_count(&self) -> usize {
        self.inner.transactions.len()
    }
}

#[derive(Clone)]
pub struct BlockImpl {
    pub head: L2HeadImpl,
    pub entries: Vec<SimpleEntry>,
}

impl Block for BlockImpl {
    type Head = L2HeadImpl;

    fn head(&self) -> &Self::Head {
        &self.head
    }
}

impl From<BlockPayloadImpl> for BlockImpl {
    fn from(value: BlockPayloadImpl) -> Self {
        Self {
            head: value.head,
            entries: value.entries,
        }
    }
}
