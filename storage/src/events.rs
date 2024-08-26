use crossbeam_channel::Receiver;

pub struct EventsHub {
    pub ledger_signal_receiver: Option<Receiver<bool>>,
}
