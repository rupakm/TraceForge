use std::fmt::Debug;

use traceforge::thread::*;
use traceforge::*;

/// This is showing one way to model a job queue.
///
/// This abstraction comes up in many places, both actual job queues such as SQS message consumers
/// or web servers responding to HTTP requests, etc.
///
/// The central feature is that there is a sequence of incoming jobs that are distributed
/// in a nondeterministic way across a set of workers. Each worker works only one job at a time
/// and while working a job, it can create more work (submit another job.)
///
/// In almost all realistic scenarios the jobs are unordered, and the order in which they are
/// worked does not depend on the order in which they were submitted. In fact, we are looking
/// for counterexamples that might occur from working the jobs in unexpected orders or when
/// interleaved in unexpected ways.
///
/// When the workers work the jobs, this can result in the creation of more jobs, and it can also
/// have multiple non-atomic side effects. Therefore, the concurrency of jobs is also relevant
/// and counts as part of the space of behavior to explore.
///
/// This file contains several ways of modeling a job queue
/// 1. Push: every send operation delivers a message to a worker chosen by nondet()
/// 2. Push thread queue: same as previous, but the messages go through shared queue first.
/// 3. Pull model: each worker polls a shared queue to retrieve the next message to work on.

const NUM_WORKERS: usize = 2;

const INIT_TAG: u32 = 0;
const REQUEST_TAG: u32 = 1;
const SUBMIT_TAG: u32 = 2;

#[derive(Clone, Debug, PartialEq)]
enum Job {
    TopLevelJob(String),
    LeafJob(String),
}

trait JobQueue {
    fn submit_job(&self, job: Job) -> ();
}

/// --------------------------
/// Implementation 1: Push job queue: every call to submit_job nondeterministically chooses a worker
/// and sends directly to that worker.
#[derive(Clone, Debug, PartialEq)]
struct PushJobQueue {
    workers: Vec<ThreadId>,
}

impl JobQueue for PushJobQueue {
    fn submit_job(&self, job: Job) {
        let size = self.workers.len();
        let index = (0..size).nondet();
        let worker = self.workers.get(index).unwrap();
        send_msg(worker.clone(), job);
    }
}

fn create_job_queue(num_workers: usize) -> PushJobQueue {
    let mut join_handles: Vec<JoinHandle<()>> = Vec::new();
    // These worker threads are all the same, so they would ideally be spawned with spawn_symmetric,
    // but there is not currently a way to combine spawn_symmetric and spawn_daemon
    join_handles.resize_with(num_workers, || {
        spawn_daemon(push_worker_thread::<PushJobQueue>)
    });

    let workers = join_handles
        .iter()
        .map(|a| a.thread().id())
        .collect::<Vec<_>>();
    let job_queue = PushJobQueue {
        workers: workers.clone(),
    };

    // Tell all the workers about each other.
    for thread_id in &workers {
        send_tagged_msg(*thread_id, INIT_TAG, job_queue.clone());
    }

    return job_queue;
}

fn push_worker_thread<T: JobQueue + Clone + Debug + PartialEq + Send + 'static>() -> () {
    let job_queue: T = recv_tagged_msg_block(|_, t| t.is_some() && t.unwrap() == INIT_TAG);
    loop {
        match recv_msg_block() {
            Job::TopLevelJob(name) => {
                // When handling a top level job, submit two leaf jobs.
                job_queue.submit_job(Job::LeafJob(name.clone() + "1"));
                job_queue.submit_job(Job::LeafJob(name + "2"));
            }
            Job::LeafJob(..) => {
                // Do some work.
            }
        }
    }
}

#[test]
fn push_test() -> () {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(traceforge::SchedulePolicy::LTR)
            .with_verbose(1)
            .with_trace_out("/tmp/sharedmem.traces")
            .build(),
        || {
            let job_queue = create_job_queue(NUM_WORKERS);

            // Submit one top-level job.
            job_queue.submit_job(Job::TopLevelJob(String::from("a")));
        },
    );
    println!("Number of executions explored {}", stats.execs);
    println!("Number of blocked executions explored {}", stats.block);
    // This has eight executions because each of 3 jobs that are sent can go to one of 2 workers.
    // So 2 ^ 3 == 8
    assert_eq!(stats.execs, 8);
    assert_eq!(stats.block, 0);
}

/// --------------------------------------------------------------------------------------
/// Implementation 2: push with thread
/// This implementation uses a thread, so there is no nondeterminism in the sender threads.
#[derive(Clone, Debug, PartialEq)]
struct PushWithThreadJobQueue {
    queue_thread: ThreadId,
    workers: Vec<ThreadId>,
}

impl JobQueue for PushWithThreadJobQueue {
    fn submit_job(&self, job: Job) {
        send_msg(self.queue_thread, job);
    }
}

fn create_push_with_thread_job_queue(num_workers: usize) -> PushWithThreadJobQueue {
    let mut join_handles: Vec<JoinHandle<()>> = Vec::new();
    // These worker threads are all the same, so they would ideally be spawned with spawn_symmetric,
    // but there is not currently a way to combine spawn_symmetric and spawn_daemon.
    join_handles.resize_with(num_workers, || {
        spawn_daemon(push_worker_thread::<PushWithThreadJobQueue>)
    });

    let workers = join_handles
        .iter()
        .map(|a| a.thread().id())
        .collect::<Vec<_>>();

    let queue_thread_id = spawn_daemon(|| {
        let workers: Vec<ThreadId> =
            recv_tagged_msg_block(|_, t| t.is_some() && t.unwrap() == INIT_TAG);
        loop {
            let msg: Job = recv_msg_block();
            let index = (0..workers.len()).nondet();
            send_msg(*workers.get(index).unwrap(), msg);
        }
    })
    .thread()
    .id();
    send_tagged_msg(queue_thread_id, INIT_TAG, workers.clone());

    let job_queue = PushWithThreadJobQueue {
        queue_thread: queue_thread_id,
        workers: workers.clone(),
    };

    // Tell all the workers about each other.
    for thread_id in &workers {
        send_tagged_msg(*thread_id, INIT_TAG, job_queue.clone());
    }

    return job_queue;
}

#[test]
fn push_with_thread_test() -> () {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(traceforge::SchedulePolicy::LTR)
            .with_verbose(1)
            .with_trace_out("/tmp/sharedmem.traces")
            .build(),
        || {
            let job_queue = create_push_with_thread_job_queue(NUM_WORKERS);

            // Submit one top-level job.
            job_queue.submit_job(Job::TopLevelJob(String::from("a")));
        },
    );
    println!("Number of executions explored {}", stats.execs);
    println!("Number of blocked executions explored {}", stats.block);
    // This has eight executions because each of 3 jobs that are sent can go to one of 2 workers.
    // So 2 ^ 3 == 8
    // This is despite the introduction of an extra thread which handles the queue. Since there
    // are no choices to make about when the queue is executed or when the messages are delivered
    // this is the same as the previous case. It's nice to see that Must is powerful enough to
    // make this a "zero cost" abstraction in the sense that adding a new thread can have no effect
    // on the number of executions.
    assert_eq!(stats.execs, 8);
    assert_eq!(stats.block, 0);
}

///----------------------------------------------------------------------------------------------
/// Implementation 3: pull queue
/// There is a queue thread which collects the job messages, but the queue thread only sends
/// them after a worker thread requests a message. This has the benefit that there is only
/// schedule nondeterminism.

#[derive(Clone, Debug, PartialEq)]
struct PullJobQueue {
    queue_thread: ThreadId,
}

#[derive(Clone, Debug, PartialEq)]
struct WorkRequest {
    requester: ThreadId,
}

impl JobQueue for PullJobQueue {
    fn submit_job(&self, job: Job) {
        send_tagged_msg(self.queue_thread, SUBMIT_TAG, job);
    }
}

fn create_pull_job_queue(num_workers: usize) -> PullJobQueue {
    let mut join_handles: Vec<JoinHandle<()>> = Vec::new();
    // These worker threads are all the same, so they would ideally be spawned with spawn_symmetric,
    // but there is not currently a way to combine spawn_symmetric and spawn_daemon
    join_handles.resize_with(num_workers, || spawn_daemon(pull_worker_thread));

    let workers = join_handles
        .iter()
        .map(|a| a.thread().id())
        .collect::<Vec<_>>();

    let queue_thread_id = spawn_daemon(|| {
        loop {
            // Instead of actually materializing the messages in a queue, we deliberately
            // try to receive the fewest possible messages, letting all of the jobs sit in
            // the Must thread's queue.
            let WorkRequest { requester } =
                recv_tagged_msg_block(|_, t| t.is_some() && t.unwrap() == REQUEST_TAG);
            let job: Job = recv_tagged_msg_block(|_, t| t.is_some() && t.unwrap() == SUBMIT_TAG);
            send_msg(requester, job);
        }
    })
    .thread()
    .id();

    let job_queue = PullJobQueue {
        queue_thread: queue_thread_id,
    };

    // Tell all the workers about the queue
    for thread_id in &workers {
        send_tagged_msg(*thread_id, INIT_TAG, job_queue.clone());
    }

    return job_queue;
}

fn pull_worker_thread() -> () {
    let job_queue: PullJobQueue =
        recv_tagged_msg_block(|_, t| t.is_some() && t.unwrap() == INIT_TAG);
    loop {
        send_tagged_msg(
            job_queue.queue_thread,
            REQUEST_TAG,
            WorkRequest {
                requester: current().id(),
            },
        );
        match recv_msg_block() {
            Job::TopLevelJob(name) => {
                // When handling a top level job, submit two leaf jobs.
                job_queue.submit_job(Job::LeafJob(name.clone() + "1"));
                job_queue.submit_job(Job::LeafJob(name + "2"));
            }
            Job::LeafJob(..) => {
                // Do some work.
            }
        }
    }
}

#[test]
fn pull_test() -> () {
    let stats = traceforge::verify(
        Config::builder()
            .with_policy(traceforge::SchedulePolicy::LTR)
            .with_verbose(1)
            .with_trace_out("/tmp/sharedmem.traces")
            .build(),
        || {
            let job_queue = create_pull_job_queue(NUM_WORKERS);

            // Submit one top-level job.
            job_queue.submit_job(Job::TopLevelJob(String::from("a")));
        },
    );
    println!("Number of executions explored {}", stats.execs);
    println!("Number of blocked executions explored {}", stats.block);
    // This produces 16 executions. It seems like this state space should be smaller because
    // it does not use any data nondeterminism, but it seems that the problem is that extra
    // RequestWork messages required by this protocol give more total executions than the previous
    // ways of modeling a job queue. One way to think about this is that each worker will send
    // one extra WorkRequest message over the course of the test.
    assert_eq!(stats.execs, 16);
    assert_eq!(stats.block, 0);
}
