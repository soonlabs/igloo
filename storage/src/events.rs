use crate::{history::TransactionBatchHistoryInfo, RollupStorage};
use crossbeam_channel::Receiver;

impl RollupStorage {
    pub fn on_block_complete(&self, history_info: TransactionBatchHistoryInfo) {
        self.notify_block_complete();
        self.send_transaction_history_status(history_info);
    }
}

pub struct EventsHub {
    pub ledger_signal_receiver: Option<Receiver<bool>>,
}
