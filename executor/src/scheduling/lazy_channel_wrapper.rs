use crossbeam_channel as channel;
use std::marker::PhantomData;

pub struct LazyReceiver<A, B, F>
where
    F: Fn(A) -> B,
{
    inner: channel::Receiver<A>,
    transform: F,
    _phantom: PhantomData<B>,
}

impl<A, B, F> LazyReceiver<A, B, F>
where
    F: Fn(A) -> B,
{
    pub fn new(rx: channel::Receiver<A>, transform: F) -> Self {
        LazyReceiver {
            inner: rx,
            transform,
            _phantom: PhantomData,
        }
    }

    pub fn recv(&self) -> Result<B, channel::RecvError> {
        self.inner.recv().map(|a| (self.transform)(a))
    }

    pub fn try_recv(&self) -> Result<B, channel::TryRecvError> {
        self.inner.try_recv().map(|a| (self.transform)(a))
    }
}

pub fn lazy_wrapper<A, B, F>(rx: channel::Receiver<A>, transform: F) -> LazyReceiver<A, B, F>
where
    F: Fn(A) -> B,
{
    LazyReceiver::new(rx, transform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_receiver_recv() {
        let (tx, rx) = channel::unbounded();
        let lazy_rx = lazy_wrapper(rx, |x: i32| x * 2);

        tx.send(5).unwrap();
        tx.send(10).unwrap();

        assert_eq!(lazy_rx.recv().unwrap(), 10);
        assert_eq!(lazy_rx.recv().unwrap(), 20);
    }

    #[test]
    fn test_lazy_receiver_try_recv() {
        let (tx, rx) = channel::unbounded();
        let lazy_rx = lazy_wrapper(rx, |s: String| s.len());

        assert!(lazy_rx.try_recv().is_err());

        tx.send("hello".to_string()).unwrap();
        assert_eq!(lazy_rx.try_recv().unwrap(), 5);

        tx.send("world".to_string()).unwrap();
        assert_eq!(lazy_rx.try_recv().unwrap(), 5);

        assert!(lazy_rx.try_recv().is_err());
    }
}
