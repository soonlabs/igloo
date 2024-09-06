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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_transaction() {
        let transaction_api = TransactionApi::default();
        let result: RpcResult<String> = transaction_api
            .get_transaction("signature".to_string())
            .await;
        assert_eq!(result.unwrap(), "transaction_json");
    }
}
