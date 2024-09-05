use jsonrpsee::core::RpcResult;

#[derive(Default)]
pub struct AccountApi;

impl AccountApi {
    pub async fn get_account_info(&self, account_id: String) -> RpcResult<String> {
        // Simulate fetching account info
        Ok(format!("Account info for: {}", account_id))
    }

    pub async fn get_multiple_accounts(&self, account_ids: Vec<String>) -> RpcResult<Vec<String>> {
        // Simulate fetching multiple accounts
        let results = account_ids
            .into_iter()
            .map(|id| format!("Account info for: {}", id))
            .collect();
        Ok(results)
    }
}
