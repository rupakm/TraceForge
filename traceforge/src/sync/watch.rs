//! Must's implementation of [`tokio::sync::watch`].

use futures::task::Context;
use futures::task::Poll;
use std::pin::Pin;
//use futures::task::Poll::Ready;

pub type Receiver<T> = crate::channel::Receiver<T>;

#[derive(Clone, Debug, PartialEq)]
pub struct Sender<T> {
    sender: crate::channel::Sender<T>,
}

unsafe impl<T: Send> Send for Sender<T> {}

unsafe impl<T: Sync> Sync for Sender<T> {}

impl<T: Message + 'static> Sender<T> {
    fn new(sender: crate::channel::Sender<T>) -> Self {
        Sender { sender }
    }

    #[allow(unused_mut)]
    pub fn send(mut self, v: T) -> Result<(), T> {
        self.sender.send_msg(v);
        Ok(())
    }
}

impl<T: Default> Default for Sender<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

// The current version ignores the buffer size
pub fn channel<T>() -> (Sender<T>, Receiver<T>)
where
    T: Clone + std::fmt::Debug + PartialEq + Message + 'static,
{
    let (tx, rx) = crate::channel::Builder::<T>::new().build();
    (Sender::new(tx), rx)
}

impl<T: Message + Clone + 'static> Future for Receiver<T> {
    type Output = Result<T, error::RecvError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(Ok(self.recv_msg_block()))
    }
}

pub mod error {
    //! Watch error types.

    use std::error::Error;
    use std::fmt;

    /// Error produced when sending a value fails.
    #[derive(PartialEq, Eq, Clone, Copy)]
    pub struct SendError<T>(pub T);

    // ===== impl SendError =====

    impl<T> fmt::Debug for SendError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("SendError").finish_non_exhaustive()
        }
    }

    impl<T> fmt::Display for SendError<T> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "channel closed")
        }
    }

    impl<T> Error for SendError<T> {}

    /// Error produced when receiving a change notification.
    #[derive(Debug, Eq, PartialEq, Clone)]
    pub struct RecvError(pub(super) ());

    // ===== impl RecvError =====

    impl fmt::Display for RecvError {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "channel closed")
        }
    }

    impl Error for RecvError {}
}
