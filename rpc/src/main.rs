#![allow(unused_variables)]

use jsonrpsee::server::{RpcModule, Server};
use jsonrpsee::types::Params;
use std::net::SocketAddr;
// use std::sync::Arc;
// use jsonrpsee::core::RpcResult;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_addr = run_server().await?;
    println!("Server running on http://{}", server_addr);

    futures::future::pending().await
}

async fn run_server() -> anyhow::Result<SocketAddr> {
    // Create a new RPC module
    let mut module = RpcModule::new(());

    // /ping
    module
        .register_method("say_hello", |_, _, _| "pong")
        .unwrap();
    // status
    module
        .register_method("status", |_: Params, _, _| "Server is running")
        .unwrap();

    module
        .register_method("getAccountInfo", |params, _, _| {
            // TODO: Implement actual logic
            "Account info for pubkey: "
        })
        .unwrap();

    module
        .register_method("getMultipleAccounts", |params, _, _| {
            // TODO: Implement actual logic
            "Account info for multiple pubkeys: "
        })
        .unwrap();

    module
        .register_method("getSignaturesForAddress", |params, _, _| {
            // TODO: Implement actual logic
            "Signatures for address: "
        })
        .unwrap();

    module
        .register_method("getTransaction", |params, _, _| {
            // TODO: Implement actual logic
            "Transaction info: "
        })
        .unwrap();

    module
        .register_method("getBlock", |params, _, _| {
            // TODO: Implement actual logic
            "Block info: "
        })
        .unwrap();

    module
        .register_method("getBlocks", |params, _, _| {
            // TODO: Implement actual logic
            "Blocks between slots: "
        })
        .unwrap();

    module
        .register_method("getSlot", |params, _, _| {
            // TODO: Implement actual logic
            "Current slot: "
        })
        .unwrap();

    // Create and start the HTTP server
    let server = Server::builder().build("127.0.0.1:8080").await.unwrap();
    let addr = server.local_addr()?;
    let handle = server.start(module);

    tokio::spawn(handle.stopped());

    Ok(addr)
}
