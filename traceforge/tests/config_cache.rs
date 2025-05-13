// Reproducing a P model to demonstrate differences between mailbox and P2P communication semantics
// The original P model is attached at the end
// Main actors: Applier, LMDB, and ConfigCacheService
mod utils;
use traceforge::msg::Message;
use traceforge::*;
use log::{info, warn};
use std::collections::HashMap;

/// Adding a super trait alias that encompasses all the traits we'd like messages to satisfy
/// This will compactify the code
pub trait MessageTraits: Message + Send + 'static {}

/// Nothing needs to happen here, this is just a super trait alias
impl<E> MessageTraits for E where E: Message + Send + 'static {}

pub(crate) trait Actor<M: MessageTraits> {
    // The handle function deals with a message sent to the actor
    // It can send out  messages to other actors using `traceforge::send_msg`
    fn handle(&mut self, _msg: M) {}

    // This is the ``main loop'' that will run in a thread
    fn execute(&mut self) {
        loop {
            let msg = traceforge::recv_msg_block();
            self.handle(msg);
        }
    }
}

use traceforge::thread::{JoinHandle, ThreadId};

type LogicalTimestamp = u32;
type Mid = u32;
type Key = String;
type Sender = ThreadId;

#[derive(Clone, Debug, PartialEq)]
pub enum ApplierMessage {
    ApplyDiffReq {
        sdr: Sender,
        key: Key,
        log_t: LogicalTimestamp,
    },
    CoherencyUpdateResp {
        rid: Mid,
        key: Key,
        log_t: LogicalTimestamp,
    },
    LMDBWriteResp {
        rid: Mid,
        log_t: LogicalTimestamp,
    },
}

// Applier needs to know the LMDB and CongigCacheService actors to send messages to
struct Applier {
    lmdb: ThreadId,
    ccs: ThreadId,
}

const DB_COMMIT_TAG: u32 = 42; // need to know if write has been committed

impl Actor<ApplierMessage> for Applier {
    fn handle(&mut self, m: ApplierMessage) {
        match m {
            ApplierMessage::ApplyDiffReq { sdr, key, log_t } => {
                info!("[MODEL][APPLIER] Received apply diffs for key: {key}! ");
                traceforge::send_msg(
                    self.lmdb,
                    LMDBMessage::WriteRq {
                        rid: log_t,
                        sdr: thread::current().id(),
                        key: key.clone(),
                        log_t: log_t,
                    },
                );
                info!("[MODEL][APPLIER] Sent write request to lmdb ");
                // wait for response indicating that LMDB committed the write
                // you dont need to do anything with the message received
                let _: ApplierMessage =
                    traceforge::recv_tagged_msg_block(|_, t| t == Some(DB_COMMIT_TAG));
                // let the sender know you processed the diff request
                traceforge::send_msg(sdr, EnvMessage::ApplyDiffResp { log_t: log_t });
                info!("[MODEL][APPLIER] Sent write request to main ");
                // now send update req to ccs
                traceforge::send_msg(
                    self.ccs,
                    CCSMessage::CoherencyUpdateReq {
                        rid: log_t,
                        sdr: thread::current().id(),
                        key: key.clone(),
                        log_t: log_t,
                    },
                );
                info!("[MODEL][APPLIER] Sent update request to CCS ");
            }
            ApplierMessage::CoherencyUpdateResp { .. } => {
                // ignore CoherencyUpdateResp
                warn!("[MODEL][APPLIER] Message {m:?} ignored ");
            }
            ApplierMessage::LMDBWriteResp { .. } => {
                panic!("Unexpected LMDBWriteResp received");
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LMDBMessage {
    ReadRq {
        rid: Mid,
        sdr: Sender,
        key: Key,
    },
    WriteRq {
        rid: Mid,
        sdr: Sender,
        key: Key,
        log_t: LogicalTimestamp,
    },
}

struct LMDB {
    db: HashMap<String, u32>,
}

impl Actor<LMDBMessage> for LMDB {
    fn handle(&mut self, m: LMDBMessage) {
        match m {
            LMDBMessage::ReadRq { rid, sdr, key } => {
                info!("[MODEL][DB] Read message ");
                if let Some(v) = self.db.get(&key) {
                    traceforge::send_msg(
                        sdr,
                        CCSMessage::LMDBReadResp {
                            rid: rid,
                            sdr: thread::current().id(),
                            value: *v as i32,
                        },
                    );
                } else {
                    traceforge::send_msg(
                        sdr,
                        CCSMessage::LMDBReadResp {
                            rid: rid,
                            sdr: thread::current().id(),
                            value: -1,
                        },
                    );
                }
            }
            LMDBMessage::WriteRq {
                rid,
                sdr,
                key,
                log_t,
            } => {
                info!("[MODEL][DB] Write message, logical time {log_t} ");
                if let Some(v) = self.db.get(&key) {
                    assert(log_t > *v);
                };
                self.db.insert(key, log_t);
                traceforge::send_tagged_msg(
                    sdr,
                    DB_COMMIT_TAG,
                    ApplierMessage::LMDBWriteResp {
                        rid: rid,
                        log_t: log_t,
                    },
                );
                info!("[MODEL][DB] Sent write response to applier ");
            }
        }
    }
}

// CCS messages
#[derive(Clone, Debug, PartialEq)]
pub enum CCSMessage {
    LMDBReadResp {
        rid: Mid,
        sdr: Sender,
        value: i32,
    },
    AsyncLoadReq {
        rid: Mid,
        sdr: Sender,
        key: Key,
    },
    CoherencyUpdateReq {
        rid: Mid,
        sdr: Sender,
        key: Key,
        log_t: LogicalTimestamp,
    },
}
//creating a type for load requests because they are stored in the map
struct LR {
    #[allow(unused)]
    rid: Mid,
    #[allow(unused)]
    sdr: Sender,
    key: Key,
}

struct ConfigCacheService {
    lmdb: ThreadId,
    inflight_requests: HashMap<Mid, LR>,
    config_min_logical_timestamp: HashMap<String, LogicalTimestamp>,
}

impl Actor<CCSMessage> for ConfigCacheService {
    fn handle(&mut self, m: CCSMessage) {
        match m {
            CCSMessage::LMDBReadResp { rid, sdr: _, value } => {
                info!("[MODEL][CCS] Received read response {value} ");
                if let Some(r) = self.inflight_requests.get(&rid) {
                    if let Some(t) = self.config_min_logical_timestamp.get(&r.key) {
                        info!("[MODEL][CCS] Comparing {value} with {t} ");
                        assert(value >= *t as i32);
                    }
                    self.inflight_requests.remove(&rid); // request processed
                } else {
                    panic!(
                        "[MODEL][CCS] Error - Response does not correspond to inflight requests!"
                    );
                }
            }
            CCSMessage::AsyncLoadReq { rid, sdr, key } => {
                info!("[MODEL][CCS] Received load request ");
                self.inflight_requests.insert(
                    rid,
                    LR {
                        rid: rid,
                        sdr: sdr,
                        key: key.clone(),
                    },
                );
                traceforge::send_msg(
                    self.lmdb,
                    LMDBMessage::ReadRq {
                        rid: rid,
                        sdr: thread::current().id(),
                        key: key.clone(),
                    },
                );
            }

            CCSMessage::CoherencyUpdateReq {
                rid,
                sdr,
                key,
                log_t,
            } => {
                info!("[MODEL][CCS] Received coherency update request ");
                if let Some(t) = self.config_min_logical_timestamp.get(&key) {
                    assert(log_t > *t);
                }
                let kc = key.clone();
                self.config_min_logical_timestamp.insert(key, log_t);
                traceforge::send_msg(
                    sdr,
                    ApplierMessage::CoherencyUpdateResp {
                        rid: rid,
                        key: kc,
                        log_t: log_t,
                    },
                );
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EnvMessage {
    ApplyDiffResp { log_t: LogicalTimestamp },
}
fn simple_scenario() {
    // Set up actors
    let lmdb: JoinHandle<()> = thread::spawn_daemon(move || {
        let mut d: LMDB = LMDB { db: HashMap::new() };
        d.execute();
    });

    let lmdb_tid = lmdb.thread().id();

    let ccs: JoinHandle<()> = thread::spawn_daemon(move || {
        let mut c = ConfigCacheService {
            lmdb: lmdb_tid,
            inflight_requests: HashMap::new(),
            config_min_logical_timestamp: HashMap::new(),
        };
        c.execute();
    });

    let ccs_tid = ccs.thread().id();

    let applier: JoinHandle<()> = thread::spawn_daemon(move || {
        let mut a = Applier {
            lmdb: lmdb_tid,
            ccs: ccs_tid,
        };
        a.execute();
    });

    let app_tid = applier.thread().id();

    const SV: u32 = 1000000;

    let mut iterations = 1;
    traceforge::send_msg(
        app_tid,
        ApplierMessage::ApplyDiffReq {
            sdr: thread::current().id(),
            key: String::from("d1.cloudfront.net"),
            log_t: iterations,
        },
    );
    // first time wait to initialize keys in database
    let _: EnvMessage = traceforge::recv_msg_block();
    traceforge::send_msg(
        ccs_tid,
        CCSMessage::AsyncLoadReq {
            rid: SV,
            sdr: thread::current().id(),
            key: String::from("d1.cloudfront.net"),
        },
    );

    iterations += 1;
    const MAX: u32 = 2;

    while iterations <= MAX {
        traceforge::send_msg(
            app_tid,
            ApplierMessage::ApplyDiffReq {
                sdr: thread::current().id(),
                key: String::from("d1.cloudfront.net"),
                log_t: iterations,
            },
        );
        traceforge::send_msg(
            ccs_tid,
            CCSMessage::AsyncLoadReq {
                rid: SV + iterations,
                sdr: thread::current().id(),
                key: String::from("d1.cloudfront.net"),
            },
        );
        iterations += 1;
    }
}

fn output_must_results(s: Stats) {
    let total = s.execs + s.block;
    println!("\n\n***** Total number of executions explored: {total} *****");
    let blocked = s.block;
    if blocked > 0 {
        println!("***** ({blocked} out of {total} executions were blocked) *****\n\n");
    }
}

#[test]
//This test runs the model under peer-to-peer communication semantics
fn simple_scenario_p2p() {
    // uncomment the next line to enable logging
    //init_log();
    let stats = traceforge::verify(
        Config::builder()
            // uncomment the next line to stop after error and get counterexamples
            .with_keep_going_after_error(true)
            .build(),
        simple_scenario,
    );
    output_must_results(stats);
}

// Same behaviors as causal
#[test]
//This test runs the model under mailbox communication semantics
fn simple_scenario_mailbox() {
    // uncomment the next line to enable logging
    // init_log();
    let stats = traceforge::verify(
        Config::builder().with_cons_type(ConsType::Mailbox).build(),
        simple_scenario,
    );
    output_must_results(stats);
}

// ======= ORIGINAL P MODEL  =========
// Expecting to see the following sequence of events, which would fail the safety property, but it did not happen.
// Note: the following sequence omits intermediate events for clarity.
// (Main -> CCS ->) LMDB -> CCS
// [SEND] LMDB sends eLMDBReadRsp to CCS (containing stale data) ---------------------------------.
//                                                                                                |
// (Main -> Applier ->) LMDB -> Applier - CCS                                                     |
// [SEND] LMDB sends eLMDBWriteRsp to Applier (indicating fresh data has now been comitted) --.   |
// [RECV] Applier receives eLMDBWriteRsp from LMDB <------------------------------------------'   |
// [SEND] Applier sends eCoherencyUpdateReq to CCS --------.                                      |
// [RECV] CCS receives eCoherencyUpdateReq from Applier <--'                                      |
//    ^                                                                                           |
//    |---- Out of order delivery to CCS (Expected, but P did not make it happen)                 |
//    v                                                                                           |
// [RECV] CCS receives eLMDBReadRsp from LMDB (containing stale data) <---------------------------'
// Here ^, the invalidation (aka coherency update) has already been applied
// and is not going to be applied again, leaving the stale data in the cache.
// test tcMain [main=Main]:
//     assert
//         EveryDiskWriteEventuallyGetsInvalidatedInConfigCache
//     in
//    (union
//     {
//         ConfigCacheService,  // CCS for short
//         Applier,
//         LMDB
//     },
//     {
//         Main
//     });
// machine Main {
//     var machines: tMachines;
//     start state Init {
//         entry {
//             machines = (
//                 applier = default(machine),
//                 ccs = default(machine),
//                 lmdb = default(machine)
//             );
//             machines.applier = new Applier();
//             machines.ccs = new ConfigCacheService();
//             machines.lmdb = new LMDB();
//             send machines.applier, eInitMachines, machines;
//             send machines.ccs, eInitMachines, machines;
//             send machines.lmdb, eInitMachines, machines;
//             goto SendRequests;
//         }
//     }
//     state SendRequests {
//         entry {
//             var i: int;
//             // Send diffs to applier and async load requests to config cache service (ccs).
//             // Expecting those events to interleave as they are on different channels.
//             i = 1;
//             while (i <= 2) {
//                 send machines.applier, eApplyDiffReq, (
//                     logical_timestamp = i,
//                     sender = this,
//                     key = "d1.cloudfront.net"
//                 );
//                 // All keys should be initialized in lmdb before any read requests requests.
//                 if (i == 1) {
//                     receive {
//                         case eApplyDiffRsp: (rsp: tApplyDiffRsp) {
//                             // Diff is applied before we proceed to send load requests.
//                         }
//                     }
//                 }
//                 send machines.ccs, eAsyncLoadReq, (
//                     rid = 10000000 + i,
//                     sender = this,
//                     key = "d1.cloudfront.net"
//                 );
//                 i = i + 1;
//             }
//             goto WaitForTermination;
//         }
//     }
//     state WaitForTermination {
//         ignore eApplyDiffRsp;
//     }
// }
// machine ConfigCacheService {
//     var machines: tMachines;
//     var inflight_requests: map[int, tAsyncLoadReq];
//     var config_min_logical_timestamp: map[string, int];
//     start state Init {
//         on eInitMachines do (machines0: tMachines) {
//             machines = machines0;
//             goto HandleRequests;
//         }
//     }
//     state HandleRequests {
//         on eAsyncLoadReq do (req: tAsyncLoadReq) {
//             inflight_requests += (req.rid, req);
//             send machines.lmdb, eLMDBReadReq, (rid = req.rid, sender = this, key = req.key);
//         }
//         on eLMDBReadRsp do (rsp: tLMDBReadRsp) {
//             var req: tAsyncLoadReq;
//             assert rsp.rid in inflight_requests;
//             req = inflight_requests[rsp.rid];
//             inflight_requests -= (rsp.rid);
//             // This is the safety property we want to model.
//             if (req.key in config_min_logical_timestamp) {
//                 assert rsp.value >= config_min_logical_timestamp[req.key];
//             }
//         }
//         on eCoherencyUpdateReq do (req: tCoherencyUpdateReq) {
//             if (req.key in config_min_logical_timestamp) {
//                 assert req.logical_timestamp > config_min_logical_timestamp[req.key];
//                 config_min_logical_timestamp -= (req.key);
//             }
//             config_min_logical_timestamp += (req.key, req.logical_timestamp);
//             send req.sender, eCoherencyUpdateRsp, (
//                 rid = req.rid,
//                 logical_timestamp = req.logical_timestamp,
//                 key = req.key
//             );
//         }
//     }
// }
// machine LMDB {
//     var lmdb: map[string, int];
//     start state Init {
//         on eInitMachines do (machines: tMachines) {
//             goto HandleRequests;
//         }
//     }
//     state HandleRequests {
//         on eLMDBReadReq do (req: tLMDBReadReq) {
//             send req.sender, eLMDBReadRsp, (
//                 rid = req.rid,
//                 value = lmdb[req.key],
//                 sender = this
//             );
//         }
//         on eLMDBWriteReq do (req: tLMDBWriteReq) {
//             // We only care to model the logical_timestamp, not the actual value.
//             if (req.key in lmdb) {
//                 assert req.logical_timestamp > lmdb[req.key];
//                 lmdb -= (req.key);
//             }
//             lmdb += (req.key, req.logical_timestamp);
//             send req.sender, eLMDBWriteRsp, (
//                 rid = req.rid,
//                 logical_timestamp = req.logical_timestamp
//             );
//         }
//     }
// }
// machine Applier {
//     var machines: tMachines;
//     start state Init {
//         on eInitMachines do (machines0: tMachines) {
//             machines = machines0;
//             goto ApplyDiffs;
//         }
//     }
//     state ApplyDiffs {
//         on eApplyDiffReq do (req: tApplyDiffReq) {
//             send machines.lmdb, eLMDBWriteReq, (
//                 rid = req.logical_timestamp,
//                 sender = this,
//                 key = req.key,
//                 logical_timestamp = req.logical_timestamp
//             );
//             receive {
//                 case eLMDBWriteRsp: (rsp: tLMDBWriteRsp) {
//                     // LMDB committed the write before we send the coherency update.
//                 }
//             }
//             send req.sender, eApplyDiffRsp, (
//                 logical_timestamp = req.logical_timestamp,
//             );
//             send machines.ccs, eCoherencyUpdateReq, (
//                 rid = req.logical_timestamp,
//                 sender = this,
//                 logical_timestamp = req.logical_timestamp,
//                 key = req.key
//             );
//         }
//         ignore eCoherencyUpdateRsp;
//     }
// }
// // from P docs: "just before send or annouce of an event, we deliver this event to all the monitors
// // that are observing the event and synchronously execute the monitor at that point."
// // If something is written to lmdb (eLMDBWriteRsp), it should eventually be invalidated in ccs (eCoherencyUpdateRsp).
// spec EveryDiskWriteEventuallyGetsInvalidatedInConfigCache
// observes
// eLMDBWriteRsp, eCoherencyUpdateRsp {
//     var latest_written_logical_timestamp_on_disk: int;
//     start cold state NoPendingChanges {
//         on eLMDBWriteRsp do update_high_watermark;
//     }
//     hot state PendingChanges {
//         on eLMDBWriteRsp do update_high_watermark;
//         on eCoherencyUpdateRsp do (rsp: tCoherencyUpdateRsp) {
//             assert rsp.logical_timestamp <= latest_written_logical_timestamp_on_disk;
//             if (rsp.logical_timestamp >= latest_written_logical_timestamp_on_disk) {
//                 goto NoPendingChanges;
//             }
//         }
//     }
//     fun update_high_watermark(rsp: tLMDBWriteRsp) {
//         if (rsp.logical_timestamp == latest_written_logical_timestamp_on_disk) {
//             return;
//         }
//         assert rsp.logical_timestamp > latest_written_logical_timestamp_on_disk;
//         assert rsp.logical_timestamp == latest_written_logical_timestamp_on_disk + 1;
//         latest_written_logical_timestamp_on_disk = rsp.logical_timestamp;
//         goto PendingChanges;
//     }
// }
// type tMachines = (
//     applier: machine,
//     ccs: machine,
//     lmdb: machine
// );
// type tApplyDiffReq = (
//     logical_timestamp: int,  // acts as a logical clock for spec, as well as rid
//     sender: machine,
//     key: string  // value defaults to the logical_timestamp
// );
// type tApplyDiffRsp = (
//     logical_timestamp: int
// );
// type tAsyncLoadReq = (
//     rid: int,
//     sender: machine,
//     key: string
// );
// type tLMDBReadReq = (
//     rid: int,
//     sender: machine,
//     key: string
// );
// type tLMDBReadRsp = (
//     rid: int,
//     value: int,  // -1 => not found.
//     sender: machine
// );
// type tLMDBWriteReq = (
//     rid: int,
//     sender: machine,
//     key: string,
//     logical_timestamp: int
// );
// type tLMDBWriteRsp = (
//     rid: int,
//     logical_timestamp: int
// );
// type tCoherencyUpdateReq = (
//     rid: int,
//     sender: machine,
//     logical_timestamp: int,
//     key: string
// );
// type tCoherencyUpdateRsp = (
//     rid: int,
//     logical_timestamp: int,
//     key: string
// );
// event eInitMachines: tMachines;
// event eApplyDiffReq: tApplyDiffReq;
// event eApplyDiffRsp: tApplyDiffRsp;
// event eAsyncLoadReq: tAsyncLoadReq;
// event eLMDBReadReq: tLMDBReadReq;
// event eLMDBReadRsp: tLMDBReadRsp;
// event eLMDBWriteReq: tLMDBWriteReq;
// event eLMDBWriteRsp: tLMDBWriteRsp;
// event eCoherencyUpdateReq: tCoherencyUpdateReq;
// event eCoherencyUpdateRsp: tCoherencyUpdateRsp;
