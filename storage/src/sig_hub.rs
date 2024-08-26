use crossbeam_channel::Receiver;

#[derive(Default)]
pub struct SignalHub {
    pub ledger_signal_receiver: Option<Receiver<bool>>,
    pub pruned_banks_receiver: Option<Receiver<(u64, u64)>>,
}
