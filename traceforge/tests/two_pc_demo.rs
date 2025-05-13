use std::thread::sleep;
use std::time::{Duration, Instant};

use traceforge::thread;
use traceforge::thread::ThreadId;
use traceforge::Config;

// Messages to coordinator
#[derive(Clone, PartialEq, Debug)]
pub enum CoordinatorMsg {
    Init(Vec<ThreadId>),
    Yes,
    No,
}

// Messages to participants
#[derive(Clone, PartialEq, Debug)]
pub enum ParticipantMsg {
    Prepare(ThreadId),
    Commit,
    Abort,
}

fn coordinator(ps: Vec<ThreadId>) {
    // send "prepare" messages
    ps.iter()
        .for_each(|id| traceforge::send_msg(*id, ParticipantMsg::Prepare(thread::current().id())));

    // collect responses
    let mut num_yes = 0;
    for _ in 0..ps.len() {
        let r_i: CoordinatorMsg = traceforge::recv_msg_block();
        match r_i {
            CoordinatorMsg::Yes => num_yes += 1,
            CoordinatorMsg::No => (),
            _ => panic!(),
        };
    }
    if num_yes == ps.len() {
        ps.iter()
            .for_each(|id| traceforge::send_msg(*id, ParticipantMsg::Commit));
    } else {
        ps.iter()
            .for_each(|id| traceforge::send_msg(*id, ParticipantMsg::Abort));
    }
}

fn participant() {
    let cid = if let ParticipantMsg::Prepare(id) = traceforge::recv_msg_block() {
        id
    } else {
        // Unexpected message in the protocol
        panic!();
    };

    // P1
    let response = traceforge::nondet();
    if response {
        traceforge::send_msg(cid, CoordinatorMsg::Yes);
    } else {
        traceforge::send_msg(cid, CoordinatorMsg::No);
    }

    // P2
    let action = traceforge::recv_msg_block();
    match action {
        ParticipantMsg::Commit => assert!(response),
        ParticipantMsg::Abort => (),
        _ => panic!(),
    }
}

fn scenario(num_ps: u32) {
    let mut ps = Vec::new();
    for _i in 0..num_ps {
        ps.push(thread::spawn(participant));
    }
    let pids = ps.iter().map(|tid| tid.thread().id()).collect();
    let _ = thread::spawn(move || coordinator(pids));
    //sleep(Duration::from_millis(400));
}

#[test]
fn two_pc_serial() {
    let now = Instant::now();
    let num_ps: u32 = 5;

    let stats = traceforge::verify(
        Config::builder()
            //.with_dot_out("twopc.dot")
            //.with_error_trace("twopc.bug")
            //.with_verbose(1)
            .build(),
        move || scenario(num_ps),
    );
    assert_eq!(
        stats.execs as u32,
        2_u32.pow(num_ps) * (1..=num_ps).product::<u32>()
    ); // 2^N * N!
    assert_eq!(stats.block, 0);
    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
}

#[ignore]
#[test]
#[should_panic(expected = "assertion failed")]
fn replay_from_file() {
    let num_ps = 3;
    traceforge::replay(
        move || {
            let mut ps = Vec::new();
            for _i in 0..num_ps {
                ps.push(thread::spawn(participant));
            }
            let pids = ps.iter().map(|tid| tid.thread().id()).collect();
            let _ = thread::spawn(move || coordinator(pids));
            sleep(Duration::from_millis(100));
        },
        "./twopc.bug",
    );
}

//#[ignore = "Runs too long"]
#[test]
fn two_pc_parallel() {
    let now = Instant::now();
    let num_ps: u32 = 6;
    let stats = traceforge::verify(Config::builder().with_parallel(true).build(), move || {
        let mut ps = Vec::new();
        for _i in 0..num_ps {
            ps.push(thread::spawn(participant));
        }
        let pids = ps.iter().map(|tid| tid.thread().id()).collect();
        let _ = thread::spawn(move || coordinator(pids));
    });

    assert_eq!(
        stats.execs as u32,
        2_u32.pow(num_ps) * (1..=num_ps).product::<u32>()
    ); // 2^N * N!

    assert_eq!(stats.block, 0);
    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
}

#[test]
fn two_pc_test() {
    let now = Instant::now();
    let num_ps: u32 = 6;

    let stats_vec = traceforge::parallel_test(
        Config::builder()
            .with_dot_out("twopc.dot")
            .with_error_trace("twopc.bug")
            .with_max_iterations(2)
            //.with_verbose(1)
            .build(),
        move || scenario(num_ps),
    );
    for stats in stats_vec {
        assert_eq!(
            stats.execs as u32,
            2, // 2_u32.pow(num_ps) * (1..=num_ps).product::<u32>()
        ); // 2^N * N!
        assert_eq!(stats.block, 0);
    }
    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
}
