use jsonrpsee::core::RpcResult;

#[derive(Default)]
pub struct TransactionApi;

impl TransactionApi {
    pub async fn get_transaction(&self, _signature: String) -> RpcResult<String> {
        // Parse the signature string into a Signature object
        // Fetch the transaction
        // Convert the transaction to a JSON string
        Ok("transaction_json".to_string())
    }
}
