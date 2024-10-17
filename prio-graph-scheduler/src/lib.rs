//! Solana Priority Graph Scheduler.
pub mod id_generator;
pub mod in_flight_tracker;
pub mod scheduler_error;
pub mod scheduler_messages;
pub mod scheduler_metrics;
pub mod thread_aware_account_locks;
pub mod transaction_priority_id;
pub mod transaction_state;
// pub mod scheduler_controller;
pub mod deserializable_packet;
pub mod prio_graph_scheduler;
pub mod transaction_state_container;

#[macro_use]
extern crate solana_metrics;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

/// Consumer will create chunks of transactions from buffer with up to this size.
pub const TARGET_NUM_TRANSACTIONS_PER_BATCH: usize = 64;

mod read_write_account_set;

#[cfg(test)]
mod tests {
    use {
        crate::deserializable_packet::DeserializableTxPacket,
        solana_perf::packet::Packet,
        solana_sdk::{
            hash::Hash,
            message::Message,
            sanitize::SanitizeError,
            signature::Signature,
            transaction::{
                SanitizedVersionedTransaction, VersionedTransaction,
            },
        },
        solana_short_vec::decode_shortu16_len,
        std::{cmp::Ordering, mem::size_of},
        thiserror::Error,
    };

    #[derive(Debug, Error)]
    pub enum MockDeserializedPacketError {
        #[error("ShortVec Failed to Deserialize")]
        // short_vec::decode_shortu16_len() currently returns () on error
        ShortVecError(()),
        #[error("Deserialization Error: {0}")]
        DeserializationError(#[from] bincode::Error),
        #[error("overflowed on signature size {0}")]
        SignatureOverflowed(usize),
        #[error("packet failed sanitization {0}")]
        SanitizeError(#[from] SanitizeError),
    }

    #[derive(Debug, Eq)]
    pub struct MockImmutableDeserializedPacket {
        pub original_packet: Packet,
        pub transaction: SanitizedVersionedTransaction,
        pub message_hash: Hash,
        pub is_simple_vote: bool,
    }

    impl DeserializableTxPacket for MockImmutableDeserializedPacket {
        type DeserializeError = MockDeserializedPacketError;
        fn new(packet: Packet) -> Result<Self, Self::DeserializeError> {
            let versioned_transaction: VersionedTransaction = packet.deserialize_slice(..)?;
            let sanitized_transaction =
                SanitizedVersionedTransaction::try_from(versioned_transaction)?;
            let message_bytes = packet_message(&packet)?;
            let message_hash = Message::hash_raw_message(message_bytes);
            let is_simple_vote = packet.meta().is_simple_vote_tx();

            Ok(Self {
                original_packet: packet,
                transaction: sanitized_transaction,
                message_hash,
                is_simple_vote,
            })
        }

        fn original_packet(&self) -> &Packet {
            &self.original_packet
        }

        fn transaction(&self) -> &SanitizedVersionedTransaction {
            &self.transaction
        }

        fn message_hash(&self) -> &Hash {
            &self.message_hash
        }

        fn is_simple_vote(&self) -> bool {
            self.is_simple_vote
        }
    }

    // PartialEq MUST be consistent with PartialOrd and Ord
    impl PartialEq for MockImmutableDeserializedPacket {
        fn eq(&self, other: &Self) -> bool {
            self.message_hash == other.message_hash
        }
    }

    impl PartialOrd for MockImmutableDeserializedPacket {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for MockImmutableDeserializedPacket {
        fn cmp(&self, other: &Self) -> Ordering {
            self.message_hash().cmp(other.message_hash())
        }
    }

    /// Read the transaction message from packet data
    fn packet_message(packet: &Packet) -> Result<&[u8], MockDeserializedPacketError> {
        let (sig_len, sig_size) = packet
            .data(..)
            .and_then(|bytes| decode_shortu16_len(bytes).ok())
            .ok_or(MockDeserializedPacketError::ShortVecError(()))?;
        sig_len
            .checked_mul(size_of::<Signature>())
            .and_then(|v| v.checked_add(sig_size))
            .and_then(|msg_start| packet.data(msg_start..))
            .ok_or(MockDeserializedPacketError::SignatureOverflowed(sig_size))
    }
}
