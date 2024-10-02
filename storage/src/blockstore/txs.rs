use solana_account_decoder::{
    parse_account_data::SplTokenAdditionalData,
    parse_token::{is_known_spl_token_id, token_amount_to_ui_amount_v2, UiTokenAmount},
};
use solana_measure::measure::Measure;
use solana_metrics::datapoint_debug;
use solana_runtime::bank::{Bank, TransactionBalances};
use solana_sdk::{account::ReadableAccount, pubkey::Pubkey, transaction::SanitizedTransaction};
use solana_transaction_status::{
    token_balances::TransactionTokenBalances, TransactionTokenBalance,
};
use spl_token_2022::{
    extension::StateWithExtensions,
    state::{Account, Mint},
};
use std::{borrow::Cow, collections::HashMap, sync::Arc};

#[derive(Debug, PartialEq)]
struct TokenBalanceData {
    mint: String,
    owner: String,
    ui_token_amount: UiTokenAmount,
    program_id: String,
}

pub struct CommitBatch<'a> {
    pub sanitized_txs: Cow<'a, [SanitizedTransaction]>,
    mint_decimals: HashMap<Pubkey, u8>,
    pub transaction_indexes: Vec<usize>,
}

impl<'a> CommitBatch<'a> {
    pub fn new(sanitized_txs: Cow<'a, [SanitizedTransaction]>) -> Self {
        Self {
            transaction_indexes: (0..sanitized_txs.len()).collect(),
            sanitized_txs,
            mint_decimals: Default::default(),
        }
    }

    pub fn new_with_indexes(
        sanitized_txs: Cow<'a, [SanitizedTransaction]>,
        transaction_indexes: Vec<usize>,
    ) -> Self {
        Self {
            transaction_indexes,
            sanitized_txs,
            mint_decimals: Default::default(),
        }
    }

    pub fn transactions(&self) -> &[SanitizedTransaction] {
        &self.sanitized_txs
    }

    pub fn collect_balances(&self, bank: Arc<Bank>) -> TransactionBalances {
        let mut balances: TransactionBalances = vec![];
        for transaction in self.transactions() {
            let mut transaction_balances: Vec<u64> = vec![];
            for account_key in transaction.message().account_keys().iter() {
                transaction_balances.push(bank.get_balance(account_key));
            }
            balances.push(transaction_balances);
        }
        balances
    }

    pub fn collect_token_balances(&mut self, bank: Arc<Bank>) -> TransactionTokenBalances {
        let mut balances: TransactionTokenBalances = vec![];
        let mut collect_time = Measure::start("collect_token_balances");

        let mut records = vec![];
        for transaction in self.sanitized_txs.iter() {
            let account_keys = transaction.message().account_keys();
            let has_token_program = account_keys.iter().any(is_known_spl_token_id);

            let mut record = vec![];
            if has_token_program {
                for (index, account_id) in account_keys.iter().enumerate() {
                    if transaction.message().is_invoked(index) || is_known_spl_token_id(account_id)
                    {
                        continue;
                    }

                    record.push((index, *account_id));
                }
            }
            records.push(record);
        }
        records.iter().for_each(|record| {
            let mut transaction_balances: Vec<TransactionTokenBalance> = vec![];
            for (index, account_id) in record.iter() {
                if let Some(TokenBalanceData {
                    mint,
                    ui_token_amount,
                    owner,
                    program_id,
                }) = self.collect_token_balance_from_account(bank.as_ref(), account_id)
                {
                    transaction_balances.push(TransactionTokenBalance {
                        account_index: *index as u8,
                        mint,
                        ui_token_amount,
                        owner,
                        program_id,
                    });
                }
            }
            balances.push(transaction_balances);
        });
        collect_time.stop();
        datapoint_debug!(
            "collect_token_balances",
            ("collect_time_us", collect_time.as_us(), i64),
        );
        balances
    }

    fn collect_token_balance_from_account(
        &mut self,
        bank: &Bank,
        account_id: &Pubkey,
    ) -> Option<TokenBalanceData> {
        let account = bank.get_account(account_id)?;

        if !is_known_spl_token_id(account.owner()) {
            return None;
        }

        let token_account = StateWithExtensions::<Account>::unpack(account.data()).ok()?;
        let mint = token_account.base.mint;

        let decimals = self.mint_decimals.get(&mint).cloned().or_else(|| {
            let decimals = get_mint_decimals(bank, &mint)?;
            self.mint_decimals.insert(mint, decimals);
            Some(decimals)
        })?;

        Some(TokenBalanceData {
            mint: token_account.base.mint.to_string(),
            owner: token_account.base.owner.to_string(),
            ui_token_amount: token_amount_to_ui_amount_v2(
                token_account.base.amount,
                // NOTE: Same as parsed instruction data, ledger data always uses
                // the raw token amount, and does not calculate the UI amount with
                // any consideration for interest.
                &SplTokenAdditionalData::with_decimals(decimals),
            ),
            program_id: account.owner().to_string(),
        })
    }
}

fn get_mint_decimals(bank: &Bank, mint: &Pubkey) -> Option<u8> {
    if mint == &spl_token::native_mint::id() {
        Some(spl_token::native_mint::DECIMALS)
    } else {
        let mint_account = bank.get_account(mint)?;

        if !is_known_spl_token_id(mint_account.owner()) {
            return None;
        }

        let decimals = StateWithExtensions::<Mint>::unpack(mint_account.data())
            .map(|mint| mint.base.decimals)
            .ok()?;

        Some(decimals)
    }
}
