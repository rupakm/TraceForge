use traceforge::msg::Message;
use traceforge::thread::ThreadId;
use traceforge::*;

// Adding a super trait alias that encompasses all the traits we'd like messages to satisfy
// This will compactify the code
pub trait MessageTraits: Message + Send + 'static {}

// Nothing needs to happen here, this is just a super trait alias
impl<E> MessageTraits for E where E: Message + Send + 'static {}

pub(crate) trait Actor<M: Message + Send + 'static, RetType> {
    // The handle function deals with a message sent to the actor
    // It can send out  messages to other actors using `traceforge::send_msg`
    fn handle(&mut self, _msg: M) {}

    // This can contain any additional clean up, assertion checks, etc
    fn stop(&self) -> RetType;

    // This procedure hides the mechanics of terminating an actor.
    // In the common case, the message `M` will have a case `M::Terminate`
    // and the handler function for `M::Terminate` will ensure `can_shutdown` returns true
    fn can_shutdown(&self) -> bool;

    // This is the ``main loop'' that will run in a thread
    fn execute(&mut self) -> RetType {
        while !self.can_shutdown() {
            let msg = traceforge::recv_msg_block();
            self.handle(msg);
        }

        self.stop()
    }
}

// The following protocol defines a replicated bank account.

/// A bank account is essentially an integer that denotes the balance of a client.
struct Bank {
    balance: i32,
    can_shutdown: bool,
}

/// Requests that a client can make to a bank account.
#[derive(Clone, Debug, PartialEq)]
enum Request {
    Deposit(ThreadId, i32),
    Withdraw(ThreadId, i32),
    Terminate,
}

#[derive(Clone, Debug, PartialEq)]
enum Response {
    Ok,
}

impl Actor<Request, ()> for Bank {
    fn handle(&mut self, m: Request) {
        match m {
            Request::Deposit(tid, i) => {
                self.balance += i;
                traceforge::send_msg(tid, Response::Ok);
            }
            Request::Withdraw(tid, i) => {
                traceforge::assert(self.balance >= i);
                self.balance -= i;
                traceforge::send_msg(tid, Response::Ok);
            }
            Request::Terminate => {
                self.can_shutdown = true;
            }
        }
    }

    fn can_shutdown(&self) -> bool {
        self.can_shutdown
    }

    fn stop(&self) -> () {}
}

fn safe_scenario() {
    let bank = thread::spawn(|| {
        let mut b = Bank {
            balance: 0,
            can_shutdown: false,
        };
        b.execute();
    });
    let bank_id = bank.thread().id();

    let client1 = thread::spawn(move || {
        let myid = thread::current().id();
        traceforge::send_msg(bank_id, Request::Deposit(myid, 100));
        traceforge::send_msg(bank_id, Request::Withdraw(myid, 100));
    });

    traceforge::send_msg(bank_id, Request::Terminate);
    let _ = client1.join();
}

fn unsafe_scenario() {
    let bank = thread::spawn(|| {
        let mut b = Bank {
            balance: 0,
            can_shutdown: false,
        };
        b.execute();
    });
    let bank_id = bank.thread().id();

    let client1 = thread::spawn(move || {
        let myid = thread::current().id();
        traceforge::send_msg(bank_id, Request::Deposit(myid, 200));
        traceforge::send_msg(bank_id, Request::Withdraw(myid, 100));
    });

    let client2 = thread::spawn(move || {
        let myid = thread::current().id();
        traceforge::send_msg(bank_id, Request::Withdraw(myid, 100));
    });

    traceforge::send_msg(bank_id, Request::Terminate);
    let _ = client1.join();
    let _ = client2.join();
}

#[test]
fn safe_scenario_test() {
    let stats = traceforge::verify(
        Config::builder().with_keep_going_after_error(false).build(),
        safe_scenario,
    );
    println!("Stats = {}, {}", stats.execs, stats.block);
}

#[test]
#[ignore]
fn unsafe_scenario_test() {
    let stats = traceforge::verify(
        Config::builder().with_keep_going_after_error(false).build(),
        unsafe_scenario,
    );
    println!("Stats = {}, {}", stats.execs, stats.block);
}
