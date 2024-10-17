
use solana_sdk::hash::Hash;
use solana_sdk::packet::Packet;
use solana_sdk::transaction::{SanitizedVersionedTransaction};
use std::error::Error;

/// DeserializablePacket can be deserialized from a Packet.
///
/// DeserializablePacket will be deserialized as a SanitizedTransaction
/// to be scheduled in transaction stream and scheduler.
pub trait DeserializableTxPacket: PartialEq + PartialOrd + Eq + Sized {
    type DeserializeError: Error;

    fn new(packet: Packet) -> Result<Self, Self::DeserializeError>;

    fn original_packet(&self) -> &Packet;

    /// deserialized into versionedTx, and then to SanitizedTransaction.
    fn transaction(&self) -> &SanitizedVersionedTransaction;

    fn message_hash(&self) -> &Hash;

    fn is_simple_vote(&self) -> bool;
}
