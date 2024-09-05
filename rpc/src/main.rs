#![allow(unused_variables)]

use igloo_rpc::{AccountApi, BlockApi, SlotApi, TransactionApi};
use jsonrpsee::core::RpcResult;
use jsonrpsee::server::{RpcModule, Server};
// use jsonrpsee::types::Params;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_addr = run_server().await?;
    println!("Server running on http://{}", server_addr);

    futures::future::pending().await
}

async fn run_server() -> anyhow::Result<SocketAddr> {
    let account_api = Arc::new(Mutex::new(AccountApi::default()));
    let slot_api = Arc::new(Mutex::new(SlotApi::default()));
    let block_api = Arc::new(Mutex::new(BlockApi::default()));
    let transaction_api = Arc::new(Mutex::new(TransactionApi::default()));

    let mut module = RpcModule::new(());

    // Account API methods
    module.register_async_method("getAccountInfo", move |params, _ctx, _extensions| {
        let account_api = Arc::clone(&account_api);
        async move {
            let account_api = Arc::clone(&account_api);
            let account_id: String = "foo".to_string();
            let account_api = account_api.lock().await;
            account_api.get_account_info(account_id).await
        }
    })?;

    module.register_method("getMultipleAccounts", move |_, _, _| RpcResult::Ok("lo"))?;

    // Slot API methods
    module.register_async_method("getSlot", move |params, _ctx, _extensions| {
        let slot_api = Arc::clone(&slot_api);
        async move {
            let slot_api = slot_api.lock().await;
            slot_api.get_slot().await
        }
    })?;

    // Block API methods
    module.register_async_method("getBlock", move |params, _ctx, _extensions| {
        let block_api = Arc::clone(&block_api);
        async move {
            let block_api = block_api.lock().await;
            block_api.get_block().await
        }
    })?;

    // module.register_async_method("getBlocks", move |params, _ctx, _extensions| {
    //     let block_api = Arc::clone(&block_api);
    //     async move {
    //         let block_api = block_api.lock().await;
    //         let (start_slot, end_slot): (u64, u64) = (0, 1);
    //         block_api.get_blocks(start_slot, end_slot).await
    //     }
    // })?;

    module.register_async_method("getTransaction", move |params, _ctx, _extensions| {
        let transaction_api = Arc::clone(&transaction_api);
        async move {
            let transaction_api = transaction_api.lock().await;
            let signature: String = "foo".to_string();
            transaction_api.get_transaction(signature).await
        }
    })?;

    // Create and start the HTTP server
    let server = Server::builder().build("127.0.0.1:8080").await.unwrap();
    let addr = server.local_addr()?;
    let handle = server.start(module);

    tokio::spawn(handle.stopped());

    Ok(addr)
}
