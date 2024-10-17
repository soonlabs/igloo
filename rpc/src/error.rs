use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Init json rpc error: {0}")]
    InitJsonRpc(String),

    #[error(transparent)]
    IglooStorage(#[from] igloo_storage::Error),
}
