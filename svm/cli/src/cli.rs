use std::str::FromStr;

use anyhow::Result;
use clap::Parser;
use solana_sdk::pubkey::Pubkey;

#[derive(Parser)]
pub struct Cli {
    #[clap(short = 'p', long, env = "PROGRAM_PATH")]
    pub program_path: Option<String>,

    #[clap(long, env = "PROGRAM_BUFFER")]
    pub program_buffer: Option<Vec<u8>>,

    #[clap(short, long, default_value = "", env = "CALLDATA")]
    pub calldata: String,

    #[clap(long, env = "ENABLE_V0_MESSAGE")]
    pub enable_v0_message: bool,

    #[clap(short, long, env = "ACCOUNTS")]
    pub accounts: Vec<String>,

    #[clap(long, env = "PRINT_BALANCES")]
    pub print_balances: bool,

    #[clap(short = 'm', long, env = "MEMORY_MODE")]
    pub memory_mode: bool,
}

impl Cli {
    #[allow(clippy::type_complexity)]
    pub fn parse_accounts(&self) -> Result<Vec<(Pubkey, Option<u64>, bool, bool)>> {
        let mut parsed_accounts = vec![];

        for account in &self.accounts {
            let parts: Vec<&str> = account.split(',').collect();

            if parts.is_empty() {
                return Err(anyhow::anyhow!("Account is empty"));
            }
            let pubkey = Pubkey::from_str(parts[0])?;

            let mut lamports = None;
            let mut is_signer = false;
            let mut is_writable = false;

            if parts.len() > 1 && !parts[1].is_empty() {
                lamports = Some(parts[1].parse()?);
            }

            if parts.len() > 2 && !parts[2].is_empty() {
                is_signer = parts[2].parse()?;
            }

            if parts.len() > 3 && !parts[3].is_empty() {
                is_writable = parts[3].parse()?;
            }

            parsed_accounts.push((pubkey, lamports, is_signer, is_writable));
        }

        Ok(parsed_accounts)
    }
}
