use super::cache::LargestAccountsCache;
use super::core::rpc_accounts::AccountsData;
use super::core::rpc_accounts_scan::AccountsScan;
use super::core::rpc_bank::BankData;
use super::core::rpc_full::Full;
use super::core::rpc_minimal::Minimal;
use super::core::{
    rpc_accounts, rpc_accounts_scan, rpc_bank, rpc_full, rpc_minimal, JsonRpcConfig,
    JsonRpcRequestProcessor, MAX_REQUEST_BODY_SIZE,
};
use crossbeam_channel::{unbounded, Receiver, Sender};
use jsonrpc_core::futures_util::TryStreamExt;
use jsonrpc_core::MetaIoHandler;
use jsonrpc_http_server::{
    hyper, AccessControlAllowOrigin, CloseHandle, DomainsValidation, RequestMiddleware,
    RequestMiddlewareAction, ServerBuilder,
};
use regex::Regex;
// use solana_ledger::bigtable_upload::ConfirmedBlockUploadConfig;
// use solana_ledger::bigtable_upload_service::BigTableUploadService;
use solana_ledger::blockstore::Blockstore;
use solana_perf::thread::renice_this_thread;
use solana_runtime::bank_forks::BankForks;
use solana_runtime::prioritization_fee_cache::PrioritizationFeeCache;
use solana_runtime::snapshot_archive_info::SnapshotArchiveInfoGetter;
use solana_runtime::snapshot_config::SnapshotConfig;
use solana_runtime::snapshot_utils;
use solana_sdk::exit::Exit;
use solana_sdk::genesis_config::DEFAULT_GENESIS_DOWNLOAD_PATH;
use solana_sdk::hash::Hash;
use solana_sdk::native_token::lamports_to_sol;
use solana_sdk::transaction::SanitizedTransaction;
// use solana_storage_bigtable::CredentialType;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::thread::{Builder, JoinHandle};
use tokio_util::codec::{BytesCodec, FramedRead};

const FULL_SNAPSHOT_REQUEST_PATH: &str = "/snapshot.tar.bz2";
const INCREMENTAL_SNAPSHOT_REQUEST_PATH: &str = "/incremental-snapshot.tar.bz2";
const LARGEST_ACCOUNTS_CACHE_DURATION: u64 = 60 * 60 * 2;

pub struct JsonRpcService {
    thread_hdl: JoinHandle<()>,
    close_handle: Option<CloseHandle>,
}

struct RpcRequestMiddleware {
    ledger_path: PathBuf,
    full_snapshot_archive_path_regex: Regex,
    incremental_snapshot_archive_path_regex: Regex,
    snapshot_config: Option<SnapshotConfig>,
    bank_forks: Arc<RwLock<BankForks>>,
}

impl RpcRequestMiddleware {
    pub fn new(
        ledger_path: PathBuf,
        snapshot_config: Option<SnapshotConfig>,
        bank_forks: Arc<RwLock<BankForks>>,
    ) -> Self {
        Self {
            ledger_path,
            full_snapshot_archive_path_regex: Regex::new(
                snapshot_utils::FULL_SNAPSHOT_ARCHIVE_FILENAME_REGEX,
            )
            .unwrap(),
            incremental_snapshot_archive_path_regex: Regex::new(
                snapshot_utils::INCREMENTAL_SNAPSHOT_ARCHIVE_FILENAME_REGEX,
            )
            .unwrap(),
            snapshot_config,
            bank_forks,
        }
    }

    fn redirect(location: &str) -> hyper::Response<hyper::Body> {
        hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(hyper::header::LOCATION, location)
            .body(hyper::Body::from(String::from(location)))
            .unwrap()
    }

    fn not_found() -> hyper::Response<hyper::Body> {
        hyper::Response::builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .body(hyper::Body::empty())
            .unwrap()
    }

    fn internal_server_error() -> hyper::Response<hyper::Body> {
        hyper::Response::builder()
            .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
            .body(hyper::Body::empty())
            .unwrap()
    }

    fn strip_leading_slash(path: &str) -> Option<&str> {
        path.strip_prefix('/')
    }

    fn is_file_get_path(&self, path: &str) -> bool {
        if path == DEFAULT_GENESIS_DOWNLOAD_PATH {
            return true;
        }

        if self.snapshot_config.is_none() {
            return false;
        }

        let Some(path) = Self::strip_leading_slash(path) else {
            return false;
        };

        self.full_snapshot_archive_path_regex.is_match(path)
            || self.incremental_snapshot_archive_path_regex.is_match(path)
    }

    #[cfg(unix)]
    async fn open_no_follow(path: impl AsRef<Path>) -> std::io::Result<tokio::fs::File> {
        tokio::fs::OpenOptions::new()
            .read(true)
            .write(false)
            .create(false)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .await
    }

    #[cfg(not(unix))]
    async fn open_no_follow(path: impl AsRef<Path>) -> std::io::Result<tokio::fs::File> {
        // TODO: Is there any way to achieve the same on Windows?
        tokio::fs::File::open(path).await
    }

    fn find_snapshot_file<P>(&self, stem: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        let root = if self
            .full_snapshot_archive_path_regex
            .is_match(Path::new("").join(&stem).to_str().unwrap())
        {
            &self
                .snapshot_config
                .as_ref()
                .unwrap()
                .full_snapshot_archives_dir
        } else {
            &self
                .snapshot_config
                .as_ref()
                .unwrap()
                .incremental_snapshot_archives_dir
        };
        let local_path = root.join(&stem);
        if local_path.exists() {
            local_path
        } else {
            // remote snapshot archive path
            snapshot_utils::build_snapshot_archives_remote_dir(root).join(stem)
        }
    }

    fn process_file_get(&self, path: &str) -> RequestMiddlewareAction {
        let filename = {
            let stem = Self::strip_leading_slash(path).expect("path already verified");
            match path {
                DEFAULT_GENESIS_DOWNLOAD_PATH => {
                    inc_new_counter_info!("rpc-get_genesis", 1);
                    self.ledger_path.join(stem)
                }
                _ => {
                    inc_new_counter_info!("rpc-get_snapshot", 1);
                    self.find_snapshot_file(stem)
                }
            }
        };

        let file_length = std::fs::metadata(&filename)
            .map(|m| m.len())
            .unwrap_or(0)
            .to_string();
        info!("get {} -> {:?} ({} bytes)", path, filename, file_length);
        RequestMiddlewareAction::Respond {
            should_validate_hosts: true,
            response: Box::pin(async {
                match Self::open_no_follow(filename).await {
                    Err(err) => Ok(if err.kind() == std::io::ErrorKind::NotFound {
                        Self::not_found()
                    } else {
                        Self::internal_server_error()
                    }),
                    Ok(file) => {
                        let stream =
                            FramedRead::new(file, BytesCodec::new()).map_ok(|b| b.freeze());
                        let body = hyper::Body::wrap_stream(stream);

                        Ok(hyper::Response::builder()
                            .header(hyper::header::CONTENT_LENGTH, file_length)
                            .body(body)
                            .unwrap())
                    }
                }
            }),
        }
    }

    fn health_check(&self) -> &'static str {
        // always health
        "ok"
    }
}

impl RequestMiddleware for RpcRequestMiddleware {
    fn on_request(&self, request: hyper::Request<hyper::Body>) -> RequestMiddlewareAction {
        trace!("request uri: {}", request.uri());

        if let Some(ref snapshot_config) = self.snapshot_config {
            if request.uri().path() == FULL_SNAPSHOT_REQUEST_PATH
                || request.uri().path() == INCREMENTAL_SNAPSHOT_REQUEST_PATH
            {
                // Convenience redirect to the latest snapshot
                let full_snapshot_archive_info =
                    snapshot_utils::get_highest_full_snapshot_archive_info(
                        &snapshot_config.full_snapshot_archives_dir,
                    );
                let snapshot_archive_info =
                    if let Some(full_snapshot_archive_info) = full_snapshot_archive_info {
                        if request.uri().path() == FULL_SNAPSHOT_REQUEST_PATH {
                            Some(full_snapshot_archive_info.snapshot_archive_info().clone())
                        } else {
                            snapshot_utils::get_highest_incremental_snapshot_archive_info(
                                &snapshot_config.incremental_snapshot_archives_dir,
                                full_snapshot_archive_info.slot(),
                            )
                            .map(|incremental_snapshot_archive_info| {
                                incremental_snapshot_archive_info
                                    .snapshot_archive_info()
                                    .clone()
                            })
                        }
                    } else {
                        None
                    };
                return if let Some(snapshot_archive_info) = snapshot_archive_info {
                    RpcRequestMiddleware::redirect(&format!(
                        "/{}",
                        snapshot_archive_info
                            .path
                            .file_name()
                            .unwrap_or_else(|| std::ffi::OsStr::new(""))
                            .to_str()
                            .unwrap_or("")
                    ))
                } else {
                    RpcRequestMiddleware::not_found()
                }
                .into();
            }
        }

        if let Some(result) = process_rest(&self.bank_forks, request.uri().path()) {
            hyper::Response::builder()
                .status(hyper::StatusCode::OK)
                .body(hyper::Body::from(result))
                .unwrap()
                .into()
        } else if self.is_file_get_path(request.uri().path()) {
            self.process_file_get(request.uri().path())
        } else if request.uri().path() == "/health" {
            hyper::Response::builder()
                .status(hyper::StatusCode::OK)
                .body(hyper::Body::from(self.health_check()))
                .unwrap()
                .into()
        } else {
            request.into()
        }
    }
}

fn process_rest(bank_forks: &Arc<RwLock<BankForks>>, path: &str) -> Option<String> {
    match path {
        "/v0/circulating-supply" => {
            let bank = bank_forks.read().unwrap().root_bank();
            let total_supply = bank.capitalization();
            let non_circulating_supply =
                solana_runtime::non_circulating_supply::calculate_non_circulating_supply(&bank)
                    .expect("Scan should not error on root banks")
                    .lamports;
            Some(format!(
                "{}",
                lamports_to_sol(total_supply - non_circulating_supply)
            ))
        }
        "/v0/total-supply" => {
            let bank = bank_forks.read().unwrap().root_bank();
            let total_supply = bank.capitalization();
            Some(format!("{}", lamports_to_sol(total_supply)))
        }
        _ => None,
    }
}

impl JsonRpcService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rpc_addr: SocketAddr,
        config: JsonRpcConfig,
        snapshot_config: Option<SnapshotConfig>,
        bank_forks: Arc<RwLock<BankForks>>,
        blockstore: Arc<Blockstore>,
        genesis_hash: Hash,
        tx_channel: (Sender<SanitizedTransaction>, Receiver<SanitizedTransaction>),
        ledger_path: &Path,
        node_exit: Arc<RwLock<Exit>>,
        max_complete_transaction_status_slot: Arc<AtomicU64>,
        prioritization_fee_cache: Arc<PrioritizationFeeCache>,
    ) -> Result<Self, String> {
        info!("rpc bound to {:?}", rpc_addr);
        info!("rpc configuration: {:?}", config);
        let rpc_threads = 1.max(config.rpc_threads);
        let rpc_niceness_adj = config.rpc_niceness_adj;

        let largest_accounts_cache = Arc::new(RwLock::new(LargestAccountsCache::new(
            LARGEST_ACCOUNTS_CACHE_DURATION,
        )));

        // sadly, some parts of our current rpc implemention block the jsonrpc's
        // _socket-listening_ event loop for too long, due to (blocking) long IO or intesive CPU,
        // causing no further processing of incoming requests and ultimatily innocent clients timing-out.
        // So create a (shared) multi-threaded event_loop for jsonrpc and set its .threads() to 1,
        // so that we avoid the single-threaded event loops from being created automatically by
        // jsonrpc for threads when .threads(N > 1) is given.
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(rpc_threads)
                .on_thread_start(move || renice_this_thread(rpc_niceness_adj).unwrap())
                .thread_name("solRpcEl")
                .enable_all()
                .build()
                .expect("Runtime"),
        );

        let exit_bigtable_ledger_upload_service = Arc::new(AtomicBool::new(false));

        // Note: Since block_commitment_cache is not used, so we can not construct
        // bigtable_ledger_upload_service now.
        // TODO: support bigtable ledger storage in future
        // support bigtable ledger storage in future
        // let (bigtable_ledger_storage, _bigtable_ledger_upload_service) =
        //     if let Some(RpcBigtableConfig {
        //         enable_bigtable_ledger_upload,
        //         ref bigtable_instance_name,
        //         ref bigtable_app_profile_id,
        //         timeout,
        //         max_message_size,
        //     }) = config.rpc_bigtable_config
        //     {
        //         let bigtable_config = solana_storage_bigtable::LedgerStorageConfig {
        //             read_only: !enable_bigtable_ledger_upload,
        //             timeout,
        //             credential_type: CredentialType::Filepath(None),
        //             instance_name: bigtable_instance_name.clone(),
        //             app_profile_id: bigtable_app_profile_id.clone(),
        //             max_message_size,
        //         };
        //         runtime
        //             .block_on(solana_storage_bigtable::LedgerStorage::new_with_config(
        //                 bigtable_config,
        //             ))
        //             .map(|bigtable_ledger_storage| {
        //                 info!("BigTable ledger storage initialized");
        //
        //                 let bigtable_ledger_upload_service = if enable_bigtable_ledger_upload {
        //                     Some(Arc::new(BigTableUploadService::new_with_config(
        //                         runtime.clone(),
        //                         bigtable_ledger_storage.clone(),
        //                         blockstore.clone(),
        //                         block_commitment_cache.clone(),
        //                         max_complete_transaction_status_slot.clone(),
        //                         Arc::new(AtomicU64::new(u64::MAX)), // Actually we do not need this
        //                         ConfirmedBlockUploadConfig::default(),
        //                         exit_bigtable_ledger_upload_service.clone(),
        //                     )))
        //                 } else {
        //                     None
        //                 };
        //
        //                 (
        //                     Some(bigtable_ledger_storage),
        //                     bigtable_ledger_upload_service,
        //                 )
        //             })
        //             .unwrap_or_else(|err| {
        //                 error!("Failed to initialize BigTable ledger storage: {:?}", err);
        //                 (None, None)
        //             })
        //     } else {
        //         (None, None)
        //     };

        let full_api = config.full_api;
        let max_request_body_size = config
            .max_request_body_size
            .unwrap_or(MAX_REQUEST_BODY_SIZE);
        let request_processor = JsonRpcRequestProcessor::new(
            config,
            snapshot_config.clone(),
            bank_forks.clone(),
            blockstore,
            node_exit.clone(),
            genesis_hash,
            tx_channel,
            None, // bigtable_ledger_storage,
            largest_accounts_cache,
            max_complete_transaction_status_slot,
            prioritization_fee_cache,
        );

        let ledger_path = ledger_path.to_path_buf();

        let (close_handle_sender, close_handle_receiver) = unbounded();
        let thread_hdl = Builder::new()
            .name("solJsonRpcSvc".to_string())
            .spawn(move || {
                renice_this_thread(rpc_niceness_adj).unwrap();

                let mut io = MetaIoHandler::default();

                io.extend_with(rpc_minimal::MinimalImpl.to_delegate());
                if full_api {
                    io.extend_with(rpc_bank::BankDataImpl.to_delegate());
                    io.extend_with(rpc_accounts::AccountsDataImpl.to_delegate());
                    io.extend_with(rpc_accounts_scan::AccountsScanImpl.to_delegate());
                    io.extend_with(rpc_full::FullImpl.to_delegate());
                }

                let request_middleware =
                    RpcRequestMiddleware::new(ledger_path, snapshot_config, bank_forks.clone());
                let server = ServerBuilder::with_meta_extractor(
                    io,
                    move |req: &hyper::Request<hyper::Body>| {
                        let xbigtable = req.headers().get("x-bigtable");
                        if xbigtable.is_some_and(|v| v == "disabled") {
                            request_processor.clone_without_bigtable()
                        } else {
                            request_processor.clone()
                        }
                    },
                )
                .event_loop_executor(runtime.handle().clone())
                .threads(1)
                .cors(DomainsValidation::AllowOnly(vec![
                    AccessControlAllowOrigin::Any,
                ]))
                .cors_max_age(86400)
                .request_middleware(request_middleware)
                .max_request_body_size(max_request_body_size)
                .start_http(&rpc_addr);

                if let Err(e) = server {
                    warn!(
                        "JSON RPC service unavailable error: {:?}. \n\
                           Also, check that port {} is not already in use by another application",
                        e,
                        rpc_addr.port()
                    );
                    close_handle_sender.send(Err(e.to_string())).unwrap();
                    return;
                }

                let server = server.unwrap();
                close_handle_sender.send(Ok(server.close_handle())).unwrap();
                server.wait();
                exit_bigtable_ledger_upload_service.store(true, Ordering::Relaxed);
            })
            .unwrap();

        let close_handle = close_handle_receiver.recv().unwrap()?;
        let close_handle_ = close_handle.clone();
        node_exit.write().unwrap().register_exit(Box::new(move || {
            close_handle_.close();
        }));
        Ok(Self {
            thread_hdl,
            close_handle: Some(close_handle),
        })
    }

    pub fn exit(&mut self) {
        if let Some(c) = self.close_handle.take() {
            c.close()
        }
    }

    pub fn join(mut self) -> thread::Result<()> {
        self.exit();
        self.thread_hdl.join()
    }
}
