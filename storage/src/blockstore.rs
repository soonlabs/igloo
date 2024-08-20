use solana_ledger::blockstore_processor;

use crate::{Error, Result, RollupStorage};

impl RollupStorage {
    pub(crate) fn aligne_blockstore_with_bank_forks(&self) -> Result<()> {
        blockstore_processor::process_blockstore_from_root(
            &self.blockstore,
            &self.bank_forks,
            &self.leader_schedule_cache,
            &self.process_options,
            None,
            None,
            None,
            &self.background_service.accounts_background_request_sender,
        )
        .map_err(|err| Error::InitCommon(format!("Failed to load ledger: {err:?}")))?;
        Ok(())
    }
}
