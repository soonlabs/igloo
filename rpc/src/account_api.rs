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

#[cfg(test)]
mod tests {
    use super::*;
    use solana_account_decoder::{UiAccount, UiAccountEncoding};
    use solana_sdk::account::AccountSharedData;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn test_account_to_ui_account() {
        // Create a Pubkey for the account owner
        let owner = Pubkey::new_unique();

        // Create some sample account data
        let data = vec![1, 2, 3, 4, 5];

        // Create an AccountSharedData instance
        let mut account = AccountSharedData::new(
            1_000_000_000, // lamports
            data.len(),    // space
            &owner,        // owner
        );
        account.set_data_from_slice(&data);

        // Convert AccountSharedData to UiAccount
        let account_pubkey = Pubkey::new_unique();
        let ui_account = UiAccount::encode(
            &account_pubkey,
            &account,
            UiAccountEncoding::Base64,
            None,
            None,
        );

        // Assert properties of the UiAccount
        assert_eq!(ui_account.lamports, 1_000_000_000);
        assert_eq!(ui_account.owner, owner.to_string());
        assert!(!ui_account.executable);
        assert_eq!(ui_account.rent_epoch, 0);

        // Check the pubkey
        // assert_eq!(ui_account., account_pubkey.to_string());

        // Verify the space (data length)
        // assert_eq!(ui_account.space, data.len() as u64);
    }

    #[tokio::test]
    async fn test_get_account_info() {
        // Create the AccountApi with the mocked client
        let account_api = AccountApi;

        // Call get_account_info
        let result = account_api
            .get_account_info("TestPubkey11111111111111111111111111111".to_string())
            .await
            .unwrap();
        assert_eq!(
            result,
            "Account info for: TestPubkey11111111111111111111111111111"
        );
    }
}
