use solana_prio_graph_scheduler::deserializable_packet::DeserializableTxPacket;
use solana_program::hash::Hash;
use solana_sdk::transaction::{SanitizedTransaction, SanitizedVersionedTransaction};
use std::cmp::Ordering;
use solana_sdk::packet::Packet;


mod scheduler;
pub mod status_slicing;
pub mod stopwatch;
mod lazy_channel_wrapper;

/// Represents a scheduled transaction with additional metadata
///
/// This struct encapsulates a SanitizedTransaction along with a unique ID and priority,
/// allowing for more efficient tracking and prioritization in the scheduling process.
#[derive(Debug, Clone)]
pub struct ScheduledTransaction {
    /// The unique identifier for this transaction
    pub id: u64,
    /// The priority of this transaction in the scheduling queue
    pub priority: u64,
    /// The actual transaction data
    pub transaction: SanitizedVersionedTransaction,
}

impl PartialEq for ScheduledTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl PartialOrd for ScheduledTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Eq for ScheduledTransaction {}

impl DeserializableTxPacket for ScheduledTransaction {
    type DeserializeError = ();

    fn new(packet: Packet) -> Result<Self, Self::DeserializeError> {
        todo!()
    }
    fn transaction(&self) -> &SanitizedVersionedTransaction {
        &self.transaction
    }

    fn message_hash(&self) -> &Hash {
        todo!()
    }

    fn is_simple_vote(&self) -> bool {
        todo!()
    }
}
