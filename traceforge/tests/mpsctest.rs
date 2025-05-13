//! mpsc implementation from Membrain
//!
use traceforge::*;
use traceforge::{msg::Message, thread::*};
use std::marker::PhantomData;

pub fn channel<T>() -> (Sender<T>, Receiver<T>)
where
    T: Clone + std::fmt::Debug + PartialEq + Message + Send + Sync + 'static,
{
    let tid = thread::Builder::new()
        .name("mpsc".to_string())
        .spawn_daemon(|| channel_mgr::<T>(None))
        .expect("spawn should not fail");
    let s = Sender::<T>::new(tid.thread().id());
    let r = Receiver::<T>::new(tid.thread().id());
    (s, r)
}

#[derive(Clone, Debug, PartialEq)]
enum ChannelMsgRequest<T: Message> {
    InQ(ThreadId, T),
    DeQ(ThreadId),
    #[allow(unused)]
    DeQNonblock(ThreadId),
    #[allow(unused)]
    DropRx,
    #[allow(unused)]
    IsEmpty(ThreadId),
}

#[derive(Clone, Debug, PartialEq)]
enum ChannelMsgResponse<T: Message> {
    InQOk,
    InQError,
    DeQOk(T),
    DeQNonblockOk(Option<T>),
    //EmptyYes,
    //EmptyNo,
}

const RECV_TAG: u32 = 1;
const SEND_TAG: u32 = 2;

fn channel_mgr<T>(_bound: Option<usize>)
where
    T: Clone + std::fmt::Debug + PartialEq + Message + 'static,
{
    loop {
        // The design is that we'll used tagged messages. We'll make the tag predicate as strict
        // as possible. For example, until there is a receiver, then we won't even accept any messages
        // from the sender.
        let msg_from_receiver: ChannelMsgRequest<T> =
            recv_tagged_msg_block(|_, tag| tag == Some(RECV_TAG));
        match msg_from_receiver {
            ChannelMsgRequest::DropRx => break,
            ChannelMsgRequest::InQ(_, _) => unreachable!(),
            ChannelMsgRequest::DeQ(receiver) => {
                let msg_from_sender: ChannelMsgRequest<T> =
                    recv_tagged_msg_block(|_, tag| tag == Some(SEND_TAG));
                match msg_from_sender {
                    ChannelMsgRequest::InQ(sender, m) => {
                        send_msg(sender, ChannelMsgResponse::<T>::InQOk);
                        send_msg(receiver, ChannelMsgResponse::<T>::DeQOk(m));
                    }
                    ChannelMsgRequest::DeQ(_) => unreachable!(),
                    ChannelMsgRequest::DeQNonblock(_) => unreachable!(),
                    ChannelMsgRequest::DropRx => unreachable!(),
                    ChannelMsgRequest::IsEmpty(_) => unreachable!(),
                }
            }
            ChannelMsgRequest::DeQNonblock(receiver) => {
                let msg_from_sender: Option<ChannelMsgRequest<T>> =
                    recv_tagged_msg(|_, tag| tag == Some(SEND_TAG));
                if let Some(msg_from_sender) = msg_from_sender {
                    match msg_from_sender {
                        ChannelMsgRequest::InQ(sender, m) => {
                            send_msg(sender, ChannelMsgResponse::<T>::InQOk);
                            send_msg(receiver, ChannelMsgResponse::<T>::DeQNonblockOk(Some(m)));
                        }
                        ChannelMsgRequest::DeQ(_) => unreachable!(),
                        ChannelMsgRequest::DeQNonblock(_) => unreachable!(),
                        ChannelMsgRequest::DropRx => unreachable!(),
                        ChannelMsgRequest::IsEmpty(_) => unreachable!(),
                    }
                } else {
                    send_msg(receiver, ChannelMsgResponse::<T>::DeQNonblockOk(None));
                }
            }
            ChannelMsgRequest::IsEmpty(_tid) => {
                // This can't be implemented efficiently in the current design because
                // we are using Must's own message queues, but Must doesn't offer a peek() operation
                todo!()
            }
        }
    }
    // Now that the receiver has been dropped/closed, we can just throw away all remaining received messages.
    loop {
        let msg_from_sender: ChannelMsgRequest<T> =
            recv_tagged_msg_block(|_, tag| tag == Some(SEND_TAG));
        match msg_from_sender {
            ChannelMsgRequest::InQ(sender, _m) => {
                send_msg(sender, ChannelMsgResponse::<T>::InQError);
            }
            ChannelMsgRequest::DeQ(_) => unreachable!(),
            ChannelMsgRequest::DeQNonblock(_) => unreachable!(),
            ChannelMsgRequest::DropRx => unreachable!(),
            ChannelMsgRequest::IsEmpty(_) => unreachable!(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SendError<T>(pub T);

#[derive(Clone, Debug)]
pub enum TrySendError<T> {
    Full(T),
    Disconnected(T),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sender<T> {
    tid: ThreadId,
    msg_type: PhantomData<T>,
}

unsafe impl<T> Send for Sender<T> {}

impl<T> Sender<T> {
    pub(crate) fn new(tid: ThreadId) -> Self {
        Self {
            tid,
            msg_type: PhantomData,
        }
    }

    pub fn send(&self, msg: T) -> Result<(), SendError<T>>
    where
        T: Clone + std::fmt::Debug + PartialEq + Message + Send + Sync + 'static,
    {
        println!(
            "{:?} sending to mspc tid {:?}",
            traceforge::thread::current_id(),
            &self.tid
        );
        send_tagged_msg(
            self.tid,
            SEND_TAG,
            ChannelMsgRequest::InQ(thread::current().id(), msg.clone()),
        );
        let stid = self.tid.clone();
        let r: ChannelMsgResponse<T> = recv_tagged_msg_block(move |tid, _| tid == stid);
        println!(
            "{:?} sent to mspc tid {:?}",
            traceforge::thread::current_id(),
            &self.tid
        );
        match r {
            ChannelMsgResponse::InQOk => Ok(()),
            ChannelMsgResponse::InQError => Err(SendError(msg)), // channel has been dropped
            _ => {
                panic!("Sender::send: Unexpected channel response {:?}", r)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct RecvError;

#[derive(Debug)]
pub struct Receiver<T: Clone + std::fmt::Debug + PartialEq + Message + Send + Sync + 'static> {
    tid: ThreadId,
    msg_type: PhantomData<T>,
}

impl<T: Clone + std::fmt::Debug + PartialEq + Message + Send + Sync + 'static> Receiver<T> {
    pub(crate) fn new(tid: ThreadId) -> Self {
        Self {
            tid,
            msg_type: PhantomData,
        }
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        println!(
            "{:?} receiving [send] from mspc tid {:?}",
            traceforge::thread::current_id(),
            &self.tid
        );
        send_tagged_msg(
            self.tid,
            RECV_TAG,
            ChannelMsgRequest::<T>::DeQ(thread::current().id()),
        );
        println!(
            "{:?} receiving [recv] from mspc tid {:?}",
            traceforge::thread::current_id(),
            &self.tid
        );
        let stid = self.tid.clone();
        let r = recv_tagged_msg_block(move |tid, _| tid == stid);
        println!(
            "{:?} received from mspc tid {:?}",
            traceforge::thread::current_id(),
            &self.tid
        );
        match r {
            ChannelMsgResponse::DeQOk(v) => Ok(v),
            _ => Err(RecvError),
        }
    }
}

#[test]
fn simple() {
    let _ = traceforge::verify(Config::builder().build(), move || {
        let (s, r) = channel();
        traceforge::thread::spawn(move || {
            assert_eq!(r.recv().unwrap(), 1);
        });
        s.send(1).unwrap();
    });
}
