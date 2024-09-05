use solana_sdk::transaction::SanitizedTransaction;

pub struct BlockPayload {
    pub transactions: Vec<SanitizedTransaction>,
}
