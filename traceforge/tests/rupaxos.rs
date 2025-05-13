use traceforge::thread::ThreadId;
use traceforge::SchedulePolicy::*;
use traceforge::*;

/// This test is a reproduction of the buggy Paxos-like protocol
/// described in [1] (Fig. 1, Fig. 3).
///
/// [1] https://doi.org/10.1145/3428278

type Log = String;

#[derive(Debug)]
struct TestConfiguration {
    pub buggy: bool,
    pub help_bug_manifest: bool,
    pub num_rounds: usize,
    pub decompose: bool,
}

lazy_static::lazy_static! {
    static ref RUPAXOS_CONFIG: TestConfiguration = {
        TestConfiguration {
            buggy: if std::env::var("RUPAXOS_BUGGY").is_ok() {
        assert!(!std::env::var("RUPAXOS_NUM_ROUNDS").is_ok());
        std::env::set_var("RUPAXOS_NUM_ROUNDS", "4");
        true
            } else {
        false
            },
            help_bug_manifest: if std::env::var("RUPAXOS_HELP_BUG_MANIFEST").is_ok() {
        assert!(std::env::var("RUPAXOS_BUGGY").is_ok());
        true
            } else {
                false
            },
            num_rounds: if std::env::var("RUPAXOS_NUM_ROUNDS").is_ok() {
        std::env::var("RUPAXOS_NUM_ROUNDS").unwrap().parse().unwrap()
            } else {
                1
            },
        decompose: std::env::var("RUPAXOS_DECOMPOSE").is_ok(),
        }
    };
}

/// The `State` struct collects all local variables of [1], Fig. 1,
/// with the addition of the `nodes` vector (containing the TIDs
/// of the other system nodes), and the `output` vector
/// (representing the logs that a process outputs).
struct State {
    pub last: u64,
    pub phase: u64,
    pub leader: Option<ThreadId>,
    pub log: Log,
    pub step: Option<Round>,
    pub nodes: Vec<ThreadId>,
    pub output: Vec<Log>,
}

impl State {
    pub fn init(nodes: Vec<ThreadId>) -> Self {
        Self {
            last: 1,
            phase: 1,
            leader: None,
            log: "".to_string(),
            step: None,
            nodes: nodes,
            output: Vec::new(),
        }
    }
}

// Rust claims Prepare is never constructed? Commenting
// out for now to block the error until we can confirm.
//
#[derive(Clone, PartialEq, Debug)]
enum Round {
    // Prepare,
    Ack,
    Propose,
    Promise,
}

#[derive(Clone, PartialEq, Debug)]
enum Message {
    Init(Vec<ThreadId>),
    Prepare(Prepare),
    Ack(Ack),
    Propose(Propose),
    Promise(Promise),
    Fail,
}

#[derive(Clone, PartialEq, Debug)]
struct Prepare {
    phase: u64,
    sender: ThreadId,
}

#[derive(Clone, PartialEq, Debug)]
struct Ack {
    phase: u64,
    last: u64,
    log: Log,
}

#[derive(Clone, PartialEq, Debug)]
struct Propose {
    phase: u64,
    log: Log,
}

#[derive(Clone, PartialEq, Debug)]
struct Promise {
    phase: u64,
    log: Log,
}

fn receive(phase: u64) -> Message {
    if RUPAXOS_CONFIG.decompose {
        traceforge::recv_tagged_msg_block(move |_, t| t.is_some() && t.unwrap() == phase as u32)
    } else {
        traceforge::recv_msg_block()
    }
}

fn send(phase: u64, tid: ThreadId, msg: Message) {
    if RUPAXOS_CONFIG.decompose {
        traceforge::send_tagged_msg(tid, phase as u32, msg);
    } else {
        traceforge::send_msg(tid, msg);
    }
}

fn send_or_fail(phase: u64, tid: ThreadId, msg: Message) -> bool {
    let res = traceforge::nondet();
    send(phase, tid, if res { msg } else { Message::Fail });
    res
}

fn broadcast(state: &State, msg: Message) {
    state
        .nodes
        .iter()
        .for_each(|id| send(state.phase, id.clone(), msg.clone()));
}

fn get_leader(nodes: &Vec<ThreadId>, phase: u64) -> ThreadId {
    match phase {
        1 => nodes[0],
        2 | 3 => nodes[1],
        4 => nodes[2],
        _ => panic!(),
    }
    // nodes[phase as usize]
}

fn prepare(state: &mut State) {
    if get_leader(&state.nodes, state.phase) == thread::current().id() {
        broadcast(
            &state,
            Message::Prepare(Prepare {
                phase: state.phase + 1,
                sender: thread::current().id(),
            }),
        );
    }
    let m: Message = receive(state.phase);
    match m {
        Message::Prepare(prep) if prep.phase >= state.phase => {
            if RUPAXOS_CONFIG.buggy {
                state.last = state.phase;
            }
            state.phase = prep.phase;
            state.leader = Some(prep.sender);
            state.step = Some(Round::Ack);
        }
        _ => (),
    }
}

fn ack(state: &mut State) {
    if !matches!(state.step, Some(Round::Ack)) {
        return;
    }

    let m = Message::Ack(Ack {
        phase: state.phase,
        last: state.last,
        log: state.log.clone(),
    });
    let res = send_or_fail(state.phase, state.leader.unwrap(), m);

    if !res {
        return;
    }

    if thread::current().id() == state.leader.unwrap() {
        let mut num_acks = 0;
        let mut log = "".to_string();
        let mut last_seen = 0;
        for _i in 0..state.nodes.len() {
            let m: Message = receive(state.phase);
            match m {
                Message::Ack(ack) if ack.phase == state.phase => {
                    num_acks += 1;
                    if ack.last > last_seen {
                        last_seen = ack.last;
                        log = ack.log;
                    }
                }
                _ => (),
            }
        }
        if state.phase == 2 || state.phase == 4 {
            traceforge::assume!(num_acks > state.nodes.len() / 2);
        }
        if num_acks > state.nodes.len() / 2 {
            state.log = format!("{}{}", log, thread::current().id());
            state.step = Some(Round::Propose);
        }
    } else {
        state.step = Some(Round::Propose);
    }
}

fn propose(state: &mut State) {
    if !matches!(state.step, Some(Round::Propose)) {
        return;
    }

    if thread::current().id() == state.leader.unwrap() {
        broadcast(
            &state,
            Message::Propose(Propose {
                phase: state.phase,
                log: state.log.clone(),
            }),
        );
    }
    let m: Message = receive(state.phase);
    match m {
        Message::Propose(prop) => {
            state.log = prop.log;
            state.step = Some(Round::Promise);
            if !RUPAXOS_CONFIG.buggy {
                state.last = state.phase;
            }
        }
        _ => (),
    }
}

fn promise(state: &mut State) {
    if !matches!(state.step, Some(Round::Promise)) {
        return;
    }

    broadcast(
        &state,
        Message::Promise(Promise {
            phase: state.phase,
            log: state.log.clone(),
        }),
    );
    let mut num_proms = 0;
    for _i in 0..state.nodes.len() {
        let m: Message = receive(state.phase);
        match m {
            Message::Promise(_prom) => {
                num_proms += 1;
            }
            _ => (),
        }
    }
    if num_proms > state.nodes.len() / 2 {
        state.output.push(state.log.clone());
    }
}

fn fsm() -> Vec<Log> {
    let m = receive(0);
    let mut state = if let Message::Init(nodes) = m {
        State::init(nodes)
    } else {
        traceforge::assume!(false);
        panic!();
    };

    let num_rounds = RUPAXOS_CONFIG.num_rounds;
    for i in 0..num_rounds {
        if RUPAXOS_CONFIG.help_bug_manifest
            && thread::current().id() == state.nodes[0]
            && (i == 1 || i == 2)
        {
            continue;
        }
        if RUPAXOS_CONFIG.help_bug_manifest && thread::current().id() == state.nodes[1] && i == 3 {
            continue;
        }
        if RUPAXOS_CONFIG.help_bug_manifest && thread::current().id() == state.nodes[2] && i == 0 {
            continue;
        }
        prepare(&mut state);
        ack(&mut state);
        propose(&mut state);
        promise(&mut state);
    }
    return state.output;
}

// #[test]
// #[serial_test::serial]
// #[should_panic(expected = "assertion failed")]
// fn rupaxos_error() {
//     std::env::set_var("RUPAXOS_BUGGY", "1");
//     std::env::set_var("RUPAXOS_HELP_BUG_MANIFEST", "1");
//     let stats = traceforge::verify(
//         Config::builder()
//             .with_policy(LTR)
//             .with_cons_type(ConsType::MO)
//             .build(),
//         || {
//             lazy_static::initialize(&RUPAXOS_CONFIG);
//             let n = 3;
//             let mut nodes = Vec::new();
//             for i in 0..n {
//                 nodes.push(thread::spawn(fsm));
//             }
//             nodes.iter().for_each(|nh| {
//                 traceforge::send_msg(
//                     nh.thread().id(),
//                     Message::Init(nodes.iter().map(|tid| tid.thread().id()).collect()),
//                 )
//             });

//             let mut outs = Vec::new();
//             for h in nodes {
//                 let res = h.join().unwrap();
//                 if !res.is_empty() {
//                     outs.push(res);
//                 }
//             }
//             for out1 in &outs {
//                 for out2 in &outs {
//                     if !(out1.iter().all(|log1| {
//                         out2.iter()
//                             .find(|log2| log1.starts_with(&**log2) || log2.starts_with(&**log1))
//                             .is_some()
//                     })) {
//                         // println!("outs: {:?}", outs);
//                         assert!(false);
//                     }
//                 }
//             }
//         },
//     );
// }

#[test]
#[serial_test::serial]
fn rupaxos_ok() {
    // std::env::set_var("RUPAXOS_NUM_ROUNDS", "1");
    for cons in [ConsType::FIFO] {
        let stats = traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_cons_type(cons)
                .build(),
            || {
                lazy_static::initialize(&RUPAXOS_CONFIG);
                let n = 3;
                let mut nodes: Vec<traceforge::thread::JoinHandle<Vec<Log>>> = Vec::new();
                for _i in 0..n {
                    nodes.push(thread::spawn(fsm));
                }
                nodes.iter().for_each(|nh| {
                    traceforge::send_tagged_msg(
                        nh.thread().id(),
                        0,
                        Message::Init(nodes.iter().map(|tid| tid.thread().id()).collect()),
                    )
                });

                let mut outs = Vec::new();
                for h in nodes {
                    let res = h.join().unwrap();
                    if !res.is_empty() {
                        outs.push(res);
                    }
                }
                for out1 in &outs {
                    for out2 in &outs {
                        if !(out1.iter().all(|log1| {
                            out2.iter()
                                .find(|log2| log1.starts_with(&**log2) || log2.starts_with(&**log1))
                                .is_some()
                        })) {
                            // println!("outs: {:?}", outs);
                            assert!(false);
                        }
                    }
                }
            },
        );
        // 5184/1306 w/ fail
        assert_eq!(stats.execs, if cons == ConsType::MO { 1306 } else { 1299 });
    }
}
