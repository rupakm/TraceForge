/// A prototype example of how to build a bounded channel on top of the concurrency primitives of
/// Must.
///
use std::collections::VecDeque;

use traceforge::msg::Message;
use traceforge::thread::{self, JoinHandle, ThreadId};
use traceforge::*;

struct BlockingChannel<T: Clone + std::fmt::Debug + PartialEq> {
    content: VecDeque<T>,
    capacity: usize,

    blocked_readers: VecDeque<ThreadId>,
    blocked_writers: VecDeque<(ThreadId, T)>,
}

type ChannelId = ThreadId;

#[derive(Clone, Debug, PartialEq)]
enum BlockingChannelMsgRequest<T: Clone + std::fmt::Debug + PartialEq> {
    // requests
    InQ(ThreadId, T),
    DeQ(ThreadId),

    Terminate,
}

#[derive(Clone, Debug, PartialEq)]
enum BlockingChannelMsgResponse<T: Clone + std::fmt::Debug + PartialEq> {
    // responses
    InQOk(ChannelId),
    DeQOk(ChannelId, T),
}

impl<T: 'static + Clone + std::fmt::Debug + PartialEq + Message> BlockingChannel<T> {
    pub fn new(k: usize) -> Self {
        Self {
            content: VecDeque::new(),
            capacity: k,
            blocked_readers: VecDeque::new(),
            blocked_writers: VecDeque::new(),
        }
    }

    pub fn handle(&mut self, m: BlockingChannelMsgRequest<T>) -> bool {
        match m {
            BlockingChannelMsgRequest::InQ(tid, e) => {
                if self.content.len() < self.capacity {
                    self.content.push_back(e);
                    traceforge::send_msg(
                        tid,
                        BlockingChannelMsgResponse::<T>::InQOk(thread::current().id()),
                    );
                    while !self.blocked_readers.is_empty() && self.content.len() > 0 {
                        let tid = self.blocked_readers.pop_front().unwrap();
                        let e = self.content.pop_front().unwrap();
                        traceforge::send_msg(
                            tid,
                            BlockingChannelMsgResponse::<T>::DeQOk(thread::current().id(), e),
                        );
                    }
                } else {
                    self.blocked_writers.push_back((tid, e));
                }
                true
            }
            BlockingChannelMsgRequest::DeQ(tid) => {
                if self.content.len() > 0 {
                    let e = self.content.pop_front().unwrap();
                    traceforge::send_msg(
                        tid,
                        BlockingChannelMsgResponse::<T>::DeQOk(thread::current().id(), e),
                    );
                    while !self.blocked_writers.is_empty() && self.content.len() < self.capacity {
                        let (tid, e) = self.blocked_writers.pop_front().unwrap();
                        self.content.push_back(e);
                        traceforge::send_msg(
                            tid,
                            BlockingChannelMsgResponse::<T>::InQOk(thread::current().id()),
                        );
                    }
                } else {
                    self.blocked_readers.push_back(tid);
                }
                true
            }
            BlockingChannelMsgRequest::Terminate => false,
        }
    }
}

fn run_channel<T: Clone + std::fmt::Debug + PartialEq + Send + 'static>(
    mut ch: BlockingChannel<T>,
) {
    loop {
        let msg: BlockingChannelMsgRequest<T> = recv_msg_block();
        let b = ch.handle(msg);
        if !b {
            break;
        }
    }
}

fn wr_channel<T: Clone + std::fmt::Debug + PartialEq + Send + 'static>(
    c: ChannelId,
    who: ThreadId,
    what: T,
) {
    send_msg(c, BlockingChannelMsgRequest::InQ(who, what));
    let r: BlockingChannelMsgResponse<T> = recv_msg_block();
    match r {
        BlockingChannelMsgResponse::InQOk(ch) if ch == c => return,
        _ => traceforge::assume!(false),
    }
    return;
}

fn rd_channel<T: Clone + std::fmt::Debug + PartialEq + Send + 'static>(
    c: ChannelId,
    who: ThreadId,
) -> T {
    send_msg::<BlockingChannelMsgRequest<T>>(c, BlockingChannelMsgRequest::DeQ(who));
    let r = recv_msg_block();
    match r {
        BlockingChannelMsgResponse::DeQOk(ch, t) if ch == c => return t,
        _ => {
            traceforge::assume!(false);
            panic!("Assume false");
        }
    }
}

fn mk_channel<T: Clone + std::fmt::Debug + PartialEq + Send + 'static>(k: usize) -> JoinHandle<()> {
    let ch: BlockingChannel<T> = BlockingChannel::new(k);
    thread::spawn(move || run_channel(ch))
}

fn producer(c: ChannelId) {
    wr_channel(c, thread::current().id(), 16 as u16);
    wr_channel(c, thread::current().id(), 18 as u16);
    wr_channel(c, thread::current().id(), 20 as u16);
}

fn consumer(c: ChannelId) {
    let v: u16 = rd_channel(c, thread::current().id());
    assert_eq!(v, 16);
    let v: u16 = rd_channel(c, thread::current().id());
    assert_eq!(v, 18);
}

fn s_r_scenario() {
    let c = mk_channel::<u16>(1);
    let cid = c.thread().id();
    let producer = thread::spawn(move || producer(cid.clone()));
    let consumer = thread::spawn(move || consumer(cid.clone()));

    let _ = producer.join();
    let _ = consumer.join();
    send_msg::<_>(c.thread().id(), BlockingChannelMsgRequest::<u16>::Terminate);
    let _ = c.join();
}

#[test]
fn channel_s_r() {
    let stats = traceforge::verify(Config::default(), s_r_scenario);
    println!("Stats = {}, {}", stats.execs, stats.block);
}

#[test]
fn channel_s_r_estimate() {
    let stats = traceforge::estimate_execs_with_samples(s_r_scenario, 1_000);
    println!("Estimated state space size {}", stats);
}
