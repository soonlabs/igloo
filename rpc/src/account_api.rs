use jsonrpsee::core::RpcResult;
use solana_account_decoder::{UiAccount, UiAccountEncoding};
use solana_program::pubkey::Pubkey;
use solana_rpc_client_api::config::RpcAccountInfoConfig;
use solana_sdk::account::AccountSharedData;

#[derive(Default)]
pub struct AccountApi;

impl AccountApi {
    pub async fn get_account_info(
        &self,
        pubkey: &Pubkey,
        config: Option<RpcAccountInfoConfig>,
    ) -> RpcResult<String> {
        // Simulate fetching account info
        let RpcAccountInfoConfig {
            encoding,
            data_slice,
            commitment,
            min_context_slot,
        } = config.unwrap_or_default();

        let encoding = encoding.unwrap_or(UiAccountEncoding::Binary);

        // let account: AccountSharedData = AccountSharedData::default();
        //
        // let ui_account = UiAccount::encode(
        //     pubkey, account, encoding, None, data_slice,
        // ));
        //
        // let response = new_response!(ui_account);
        // Ok(new_response(ui_account));

        // TODO: SPL Token
        Ok(format!("Account info for: {}", pubkey.to_string()))
    }

    pub async fn get_multiple_accounts(&self, account_ids: Vec<String>) -> RpcResult<Vec<String>> {
        // Simulate fetching multiple accounts
        let results = account_ids
            .into_iter()
            .map(|id| format!("Account info for: {}", id))
            .collect();
        Ok(results)
    }

    // fn encode_account(
    //     account: &AccountSharedData,
    //     pubkey: &Pubkey,
    //     encoding: UiAccountEncoding,
    //     data_slice: Option<(usize, usize)>,
    // ) -> UiAccount {
    //     let data = account.data();
    //     let data = data_slice
    //         .map(|(offset, length)| data[offset..offset + length].to_vec())
    //         .unwrap_or_else(|| data.to_vec());
    //
    //     UiAccount {
    //         lamports: account.lamports(),
    //         owner: account.owner().to_string(),
    //         executable: account.executable(),
    //         rent_epoch: account.rent_epoch(),
    //         data: encoding.encode(data),
    //         pubkey: pubkey.to_string(),
    //         data_len: data.len() as u64,
    //     }
    // }
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
        let pubkey = Pubkey::new_from_array([0u8; 32]);
        let config = RpcAccountInfoConfig::default();
        // Call get_account_info
        let result = account_api
            .get_account_info(&pubkey, Some(config))
            .await
            .unwrap();
        assert_eq!(
            result,
            "Account info for: TestPubkey11111111111111111111111111111"
        );
    }
}
