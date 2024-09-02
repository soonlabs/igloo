use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Read,
    sync::{Arc, RwLock},
};

use igloo_interface::l2::{
    bank::{BankInfo, BankOperations},
    executor::{Config, Init},
};
use solana_sdk::{
    account::{AccountSharedData, WritableAccount},
    clock::Slot,
    hash::Hash,
    instruction::AccountMeta,
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};
use solana_svm::{
    account_loader::{CheckedTransactionDetails, TransactionCheckResult},
    transaction_processing_callback::TransactionProcessingCallback,
    transaction_processor::{
        ExecutionRecordingConfig, LoadAndExecuteSanitizedTransactionsOutput,
        TransactionBatchProcessor, TransactionProcessingConfig,
    },
};

use crate::{
    builtin::register_builtins, env::create_executable_environment,
    mock::fork_graph::MockForkGraph, prelude::*, transaction::builder::SanitizedTransactionBuilder,
};

pub struct Settings {
    pub fee_payer_balance: u64,
}

pub struct ExecutionAccounts {
    pub fee_payer: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub signatures: HashMap<Pubkey, Signature>,
}

pub struct SimpleBuilder<B: TransactionProcessingCallback + BankOperations + BankInfo> {
    bank: B,
    settings: Settings,
    tx_builder: SanitizedTransactionBuilder,
    tx_processor: Option<Arc<TransactionBatchProcessor<MockForkGraph>>>,
    fork_graph: Arc<RwLock<MockForkGraph>>,

    program_path: Option<String>,
    program_buffer: Option<Vec<u8>>,
    calldata: Vec<u8>,
    accounts: Vec<(AccountMeta, Option<AccountSharedData>)>,
    v0_message: bool,

    check_result: Option<TransactionCheckResult>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fee_payer_balance: 80000,
        }
    }
}

impl<B, C> Init for SimpleBuilder<B>
where
    B: TransactionProcessingCallback
        + BankOperations<Pubkey = Pubkey, AccountSharedData = AccountSharedData>
        + BankInfo<Hash = Hash, Pubkey = Pubkey, Slot = Slot>
        + Init<Config = C>,
    C: Config,
{
    type Error = Error;
    type Config = C;

    fn init(cfg: &Self::Config) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let bank = B::init(cfg).map_err(|e| Error::BuilderError(e.to_string()))?;
        Ok(Self {
            bank,
            settings: Default::default(),
            tx_builder: Default::default(),
            tx_processor: Default::default(),
            fork_graph: Default::default(),
            program_path: Default::default(),
            program_buffer: Default::default(),
            calldata: Default::default(),
            accounts: Default::default(),
            v0_message: Default::default(),
            check_result: Default::default(),
        })
    }
}

impl<B> SimpleBuilder<B>
where
    B: TransactionProcessingCallback
        + BankOperations<Pubkey = Pubkey, AccountSharedData = AccountSharedData>
        + BankInfo<Hash = Hash, Pubkey = Pubkey, Slot = Slot>,
{
    pub fn build(&mut self) -> Result<LoadAndExecuteSanitizedTransactionsOutput> {
        let (result, _) = self.build_ex()?;
        Ok(result)
    }

    pub fn build_ex(
        &mut self,
    ) -> Result<(
        LoadAndExecuteSanitizedTransactionsOutput,
        VersionedTransaction,
    )> {
        self.bank
            .bump()
            .map_err(|e| Error::BuilderError(e.to_string()))?;

        let buffer = self.read_program()?;
        let program_id = self
            .bank
            .deploy_program(buffer)
            .map_err(|e| Error::BuilderError(e.to_string()))?;

        let accounts = self.prepare_accounts()?;
        self.tx_builder.create_instruction(
            program_id,
            accounts.accounts,
            accounts.signatures,
            self.calldata.clone(),
        );

        let (sanitized_transaction, versioned_transaction) = self.tx_builder.build(
            self.bank.last_blockhash(),
            (accounts.fee_payer, Signature::new_unique()),
            self.v0_message,
        )?;
        let check_result = self.get_checked_tx_details();

        if self.tx_processor.is_none() {
            self.tx_processor = Some(Arc::new(create_transaction_processor(
                &mut self.bank,
                self.fork_graph.clone(),
            )?));
        }

        let processing_config = self.get_processing_config();
        Ok((
            self.tx_processor
                .as_ref()
                .ok_or(Error::TransactionProcessorIsNone)?
                .load_and_execute_sanitized_transactions(
                    &self.bank,
                    &[sanitized_transaction],
                    vec![check_result],
                    &Default::default(),
                    &processing_config,
                ),
            versioned_transaction,
        ))
    }

    pub fn settings(&mut self, settings: Settings) -> &mut Self {
        self.settings = settings;
        self
    }

    pub fn bank(&mut self, bank: B) -> &mut Self {
        self.bank = bank;
        self
    }

    pub fn get_bank(&self) -> &B {
        &self.bank
    }

    pub fn tx_processor(
        &mut self,
        tx_processor: Arc<TransactionBatchProcessor<MockForkGraph>>,
    ) -> &mut Self {
        self.tx_processor = Some(tx_processor);
        self
    }

    pub fn tx_builder(&mut self, tx_builder: SanitizedTransactionBuilder) -> &mut Self {
        self.tx_builder = tx_builder;
        self
    }

    pub fn fork_graph(&mut self, fork_graph: Arc<RwLock<MockForkGraph>>) -> &mut Self {
        self.fork_graph = fork_graph;
        self
    }

    pub fn program_path(&mut self, path: Option<String>) -> &mut Self {
        self.program_path = path;
        self
    }

    pub fn program_buffer(&mut self, buffer: Option<Vec<u8>>) -> &mut Self {
        self.program_buffer = buffer;
        self
    }

    pub fn calldata(&mut self, calldata: Vec<u8>) -> &mut Self {
        self.calldata = calldata;
        self
    }

    pub fn v0_message(&mut self, value: bool) -> &mut Self {
        self.v0_message = value;
        self
    }

    pub fn account(&mut self, meta: AccountMeta, account: Option<AccountSharedData>) -> &mut Self {
        self.accounts.push((meta, account));
        self
    }

    pub fn account_with_balance(
        &mut self,
        pubkey: Pubkey,
        lamports: Option<u64>,
        is_signer: bool,
        is_writable: bool,
    ) -> &mut Self {
        let account = if let Some(lamports) = lamports {
            let mut account = AccountSharedData::default();
            account.set_lamports(lamports);
            Some(account)
        } else {
            None
        };
        self.account(
            AccountMeta {
                pubkey,
                is_signer,
                is_writable,
            },
            account,
        )
    }

    pub fn check_result(&mut self, result: TransactionCheckResult) -> &mut Self {
        self.check_result = Some(result);
        self
    }

    fn prepare_accounts(&mut self) -> Result<ExecutionAccounts> {
        let mut accounts = vec![];
        let mut signatures = HashMap::new();
        for (meta, account) in self.accounts.iter() {
            if let Some(account) = account {
                self.bank
                    .insert_account(meta.pubkey, account.clone())
                    .map_err(|e| Error::BuilderError(e.to_string()))?;
            }

            accounts.push(meta.clone());

            if meta.is_signer {
                signatures.insert(meta.pubkey, Signature::new_unique());
            }
        }

        Ok(ExecutionAccounts {
            fee_payer: self.create_fee_payer()?,
            accounts,
            signatures,
        })
    }

    fn get_checked_tx_details(&self) -> TransactionCheckResult {
        self.check_result
            .clone()
            .unwrap_or(Ok(CheckedTransactionDetails {
                nonce: None,
                lamports_per_signature: 20,
            }))
    }

    fn create_fee_payer(&mut self) -> Result<Pubkey> {
        let fee_payer = Pubkey::new_unique();
        let mut account_data = AccountSharedData::default();
        account_data.set_lamports(self.settings.fee_payer_balance);
        self.bank
            .insert_account(fee_payer, account_data)
            .map_err(|e| Error::BuilderError(e.to_string()))?;
        Ok(fee_payer)
    }

    fn read_program(&self) -> Result<Vec<u8>> {
        if self.program_buffer.is_some() && self.program_path.is_some() {
            return Err(Error::BuilderError(
                "Both program buffer and path are set".into(),
            ));
        }

        if let Some(buffer) = self.program_buffer.clone() {
            return Ok(buffer);
        } else if let Some(path) = self.program_path.clone() {
            return self.read_file(&path);
        }

        Err(Error::BuilderError("Program not found".into()))
    }

    fn read_file(&self, dir: &str) -> Result<Vec<u8>> {
        let mut file = File::open(dir)?;
        let metadata = fs::metadata(dir)?;
        let mut buffer = vec![0; metadata.len() as usize];
        file.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    fn get_processing_config(&self) -> TransactionProcessingConfig {
        TransactionProcessingConfig {
            recording_config: ExecutionRecordingConfig {
                enable_log_recording: true,
                enable_return_data_recording: true,
                enable_cpi_recording: false,
            },
            ..Default::default()
        }
    }
}

pub fn create_transaction_processor<B>(
    bank: &mut B,
    fork_graph: Arc<RwLock<MockForkGraph>>,
) -> Result<TransactionBatchProcessor<MockForkGraph>>
where
    B: TransactionProcessingCallback + BankOperations + BankInfo<Slot = Slot>,
{
    let tx_processor = TransactionBatchProcessor::<MockForkGraph>::new(
        bank.execution_slot(),
        0, // always set epoch to 0
        HashSet::new(),
    );
    create_executable_environment(
        fork_graph.clone(),
        &mut tx_processor.program_cache.write().unwrap(),
    );

    bank.set_clock()
        .map_err(|e| Error::BuilderError(e.to_string()))?;
    tx_processor.fill_missing_sysvar_cache_entries(bank);

    register_builtins(bank, &tx_processor);

    Ok(tx_processor)
}
