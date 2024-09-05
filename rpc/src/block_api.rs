use jsonrpsee::core::RpcResult;

#[derive(Default)]
pub struct BlockApi;

impl BlockApi {
    pub async fn get_block(&self) -> RpcResult<u64> {
        // Simulate fetching the current slot
        Ok(42) // Example slot number
    }

    pub async fn get_blocks(&self, start_slot: u64, end_slot: u64) -> RpcResult<Vec<u64>> {
        Ok((start_slot..=end_slot).collect())
    }
}
