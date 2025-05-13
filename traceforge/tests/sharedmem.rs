// use crate::utils::init_log;
use traceforge::msg::Message;
use traceforge::thread::{self, current, ThreadId};
use traceforge::*;
use log::info;

mod utils;

#[derive(Clone, Debug, PartialEq)]
enum ShVarMsg<E> {
    Read(ThreadId),
    Write(ThreadId, E),
    CAS(ThreadId, E, E),

    RdOk(E),
    WrOk,
    CASResponse(bool),

    Terminate,
}

fn register<E: Clone + Copy + std::fmt::Debug + PartialEq + Send + 'static>(init: E) -> () {
    let mut register = init;
    loop {
        match traceforge::recv_msg_block() {
            ShVarMsg::Terminate => break,
            ShVarMsg::Read(tid) => traceforge::send_msg(tid, ShVarMsg::RdOk(register)),
            ShVarMsg::Write(tid, val) => {
                register = val;
                traceforge::send_msg(tid, ShVarMsg::<E>::WrOk);
            }
            ShVarMsg::CAS(tid, old, new) => {
                let mut success = false;
                if register == old {
                    register = new;
                    success = true;
                }
                traceforge::send_msg(tid, ShVarMsg::<i32>::CASResponse(success));
            }
            ShVarMsg::RdOk(_) => panic!("Val is a response"),
            ShVarMsg::WrOk => panic!("WrOk is a response"),
            ShVarMsg::CASResponse(_) => panic!("CASResponse is a response"),
        }
    }
}

fn shared_var<E: Send + 'static>(f: fn(E) -> (), init: E) -> thread::JoinHandle<()> {
    thread::spawn(move || f(init))
}

fn read<E: Clone + std::fmt::Debug + PartialEq + Message + 'static>(v: &ThreadId) -> E {
    send_msg(*v, ShVarMsg::Read::<E>(current().id()));
    let e = if let ShVarMsg::RdOk(i) = traceforge::recv_msg_block() {
        i
    } else {
        traceforge::assume!(false);
        panic!();
    };
    return e;
}

fn write<E: Clone + std::fmt::Debug + PartialEq + Message + 'static>(v: &ThreadId, e: E) -> () {
    send_msg(*v, ShVarMsg::Write::<E>(current().id(), e));
    if let ShVarMsg::<E>::WrOk = traceforge::recv_msg_block() {
        return;
    } else {
        traceforge::assume!(false);
        panic!();
    }
}

fn new_lock(init: bool) {
    let mut lock = init;
    loop {
        match traceforge::recv_msg_block() {
            ShVarMsg::Terminate => break,
            ShVarMsg::Read(_tid) => panic!("Trying to read a lock"),
            ShVarMsg::Write(_tid, _val) => panic!("Trying a write a lock without using CAS"),
            ShVarMsg::CAS(tid, old, new) => {
                let mut success = false;
                if lock == old {
                    lock = new;
                    success = true;
                }
                traceforge::send_msg(tid, ShVarMsg::<bool>::CASResponse(success));
            }
            ShVarMsg::RdOk(_) => panic!("RdOk is a response"),
            ShVarMsg::WrOk => panic!("WrOk is a response"),
            ShVarMsg::CASResponse(_) => panic!("CASResponse is a response"),
        }
    }
}

fn lock(v: &ThreadId) {
    loop {
        send_msg(*v, ShVarMsg::CAS::<bool>(current().id(), false, true));
        let e = if let ShVarMsg::CASResponse(i) = traceforge::recv_msg_block::<ShVarMsg<bool>>() {
            i
        } else {
            traceforge::assume!(false);
            panic!();
        };
        if e {
            break; // lock acquired
        } else {
            traceforge::assume!(false);
            panic!();
        }
    }
}

fn unlock(v: &ThreadId) {
    send_msg(*v, ShVarMsg::CAS::<bool>(current().id(), true, false));
}

#[derive(Clone, Debug, PartialEq)]
enum ClientMsg {
    Init(ThreadId),
    // Terminate,
}

fn client(t: bool) {
    let init = traceforge::recv_msg_block();
    let shvar = match init {
        ClientMsg::Init(shvar) => shvar,
        /* _ => {
            traceforge::assume(false);
            panic!("Not an init");
        }
        */
    };
    if t {
        // deposit
        let balance: i32 = read(&shvar);
        write(&shvar, balance + 10);
    } else {
        // withdraw
        let balance: i32 = read(&shvar);
        write(&shvar, balance - 10);
    }
}

fn add() {
    client(true);
}

fn remove() {
    client(false);
}

fn bank() {
    let account = shared_var(register, 100);
    let account_id = account.thread().id();
    let client1 = thread::spawn(add);
    let client2 = thread::spawn(remove);

    traceforge::send_msg(client1.thread().id(), ClientMsg::Init(account_id));
    traceforge::send_msg(client2.thread().id(), ClientMsg::Init(account_id));

    let _ = client1.join();
    let _ = client2.join();

    let balance: i32 = read(&account_id);
    assert_eq!(balance, 100);

    traceforge::send_msg(account_id, ShVarMsg::<i32>::Terminate);

    let _ = account.join();
}

#[derive(Clone, Debug, PartialEq)]
enum LockedClientMsg {
    Init(ThreadId, ThreadId),
    // Terminate,
}

fn client_with_lock(t: bool) {
    let init = traceforge::recv_msg_block();
    let (l, shvar) = match init {
        LockedClientMsg::Init(l, shvar) => (l, shvar),
        /*
        _ => {
            traceforge::assume(false);
            panic!("Not an init");
        }
        */
    };
    if t {
        // deposit
        lock(&l);
        let balance: i32 = read(&shvar);
        write(&shvar, balance + 10);
        unlock(&l);
    } else {
        // withdraw
        lock(&l);
        let balance: i32 = read(&shvar);
        write(&shvar, balance - 10);
        unlock(&l);
    }
}

fn add_locked() {
    client_with_lock(true);
}

fn remove_locked() {
    client_with_lock(false);
}

fn locked_bank() {
    let l = shared_var::<bool>(new_lock, false);
    let l_handle = l.thread().id();
    let r = shared_var(register, 100);
    let r_handle = r.thread().id();

    let client1 = thread::spawn(add_locked);
    let client2 = thread::spawn(remove_locked);

    traceforge::send_msg(
        client1.thread().id(),
        LockedClientMsg::Init(l_handle, r_handle),
    );
    traceforge::send_msg(
        client2.thread().id(),
        LockedClientMsg::Init(l_handle, r_handle),
    );

    let _ = client1.join();
    let _ = client2.join();

    let balance: i32 = read(&r_handle);
    info!("Balance = {balance}");
    assert_eq!(balance, 100);

    traceforge::send_msg(l_handle, ShVarMsg::<bool>::Terminate);
    traceforge::send_msg(r_handle, ShVarMsg::<i32>::Terminate);

    let _ = l.join();
    let _ = r.join();
}

fn test(fut: fn() -> ()) {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(traceforge::SchedulePolicy::LTR)
            // .with_cons_type(ConsType::FIFO)
            // .with_progress_report(true)
            // .with_symmetry(true)
            .with_verbose(1)
            .with_trace_out("/tmp/sharedmem.traces")
            // .with_dot_out("/tmp/sharedmem.dot")
            .build(),
        fut,
    );
    println!("Number of executions explored {}", stats.execs);
    println!("Number of blocked executions explored {}", stats.block);
}

#[test]
#[should_panic]
fn bank_test() {
    // init_log();
    test(bank)
}

#[test]
fn locked_bank_test() {
    // init_log();
    test(locked_bank)
}
