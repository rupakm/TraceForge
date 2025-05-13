use std::fs;
use std::io::{BufRead, ErrorKind};

use traceforge::thread::{self, current_id};
use traceforge::*;
use utils::{assert_panic_contains, assert_panic_msg};
use SchedulePolicy::*;

mod utils;

fn s_s_rr_wrong() {
    let sh1 = thread::spawn(move || {
        let r = traceforge::recv_msg();
        if r.is_some() {
            traceforge::send_msg(r.unwrap(), 1);
        }
    });
    let sh2 = thread::spawn(move || {
        let r = traceforge::recv_msg();
        if r.is_some() {
            traceforge::send_msg(r.unwrap(), 2);
        }
    });
    let rh = thread::spawn(move || {
        let m1: Option<i32> = traceforge::recv_msg();
        let m2: Option<i32> = traceforge::recv_msg();
        if m1.is_some() && m2.is_some() {
            assert!(m1.unwrap() == 2 && m2.unwrap() == 1);
        }
    });

    traceforge::send_msg(sh1.thread().id(), rh.thread().id());
    traceforge::send_msg(sh2.thread().id(), rh.thread().id());
}

/// Generate a counterexample using a function that has a deliberate failed assertion.
fn generate_trace(trace_filename: &str) {
    match fs::remove_file(trace_filename) {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        r => r.expect("Couldn't delete file"),
    }
    let result = std::panic::catch_unwind(|| {
        traceforge::verify(
            Config::builder()
                .with_policy(Arbitrary)
                .with_error_trace(trace_filename)
                .build(),
            || s_s_rr_wrong(),
        );
    });
    assert_panic_msg(
        result,
        "assertion failed: m1.unwrap() == 2 && m2.unwrap() == 1",
    );
    assert!(std::path::Path::new(trace_filename).exists());

    // Because this test deliberately generates a counterexample, if the test fails there will be
    // a lot of error info in the stdout already. Use capital letters in the following
    // assertions so that if they are triggered, they will visually stand out and direct
    // the reader towards the thing that actually causes the test to fail.
    println!("----- stack traces above this point are expected and don't indicate a problem -----");

    // Validate that the first line of the file contains "{"
    let file = std::fs::File::open(trace_filename).expect("can't open file");
    let reader = std::io::BufReader::new(file);

    match reader.lines().next() {
        None => {
            panic!("THIS IS THE ERROR: trace file {} is empty", trace_filename);
        }
        Some(Ok(line)) => {
            assert_eq!("{", line, "THIS IS THE ERROR: not valid json in trace file");
        }
        a => {
            panic!("THIS IS THE ERROR: can't read trace file; got {:?}", a);
        }
    }
}

#[test]
fn test_generate_trace() {
    let trace_filename = &{
        // Generate this with a predictable name so that you can find it when debugging tests.
        // the name must be unique per test as all tests run in parallel
        let mut p = std::env::temp_dir();
        p.push(std::path::Path::new("replaytest1.json"));
        p.to_str().unwrap().to_owned()
    };

    generate_trace(trace_filename);
}

/// Replay a counterexample. This test can't assume that the previous test executed first, so
/// it just invokes the generate_trace test first. When running the whole test suite the
/// counterexample file will be written twice, but this is a much smaller evil than results if
/// this test relies on a persistent side effect of a different test.
#[test]
fn generate_trace_and_replay() {
    // This used to be two separate tests, to generate a trace, then replay it
    // but this is sensitive to the order in which the tests execute, leading to confusion.
    // I rewrote it to do all the work in a row.
    let trace_filename = &{
        // Generate this with a predictable name so that you can find it when debugging tests.
        // the name must be unique per test as all tests run in parallel
        let mut p = std::env::temp_dir();
        p.push(std::path::Path::new("replaytest2.json"));
        p.to_str().unwrap().to_owned()
    };
    generate_trace(trace_filename);

    let result = std::panic::catch_unwind(|| {
        traceforge::replay(|| s_s_rr_wrong(), trace_filename);
    });
    assert_panic_msg(
        result,
        "assertion failed: m1.unwrap() == 2 && m2.unwrap() == 1",
    );
}

fn scenario_requiring_revisit() {
    let main_tid = current_id();

    let _t1 = {
        let main_tid = main_tid.clone();
        thread::spawn(move || {
            send_msg(main_tid, "t1".to_string());
        })
        .thread()
        .id()
    };

    let t2 = {
        let main_tid = main_tid.clone();
        let _: String = recv_msg_block();
        thread::spawn(move || {
            send_msg(main_tid, "t2".to_string());
        })
        .thread()
        .id()
    };

    let _t3 = {
        thread::spawn(move || {
            send_msg(t2, "t3".to_string());
        })
        .thread()
        .id()
    };

    let m: String = recv_msg_block();
    if m == "t2" {
        panic!("Counterexample with foobar");
    }
}

#[test]
fn verify_scenario_requiring_revisit() {
    let result = std::panic::catch_unwind(|| {
        traceforge::verify(
            Config::builder()
                .with_error_trace("/tmp/replaytest.rs_verify_scenario_requiring_revisit")
                .build(),
            scenario_requiring_revisit,
        );
    });
    assert_panic_contains(result, "foobar");

    let result = std::panic::catch_unwind(|| {
        traceforge::replay(
            scenario_requiring_revisit,
            "/tmp/replaytest.rs_verify_scenario_requiring_revisit",
        );
    });

    // This is the key assertion: the replay should be able to reach the code
    // that panics with foobar, otherwise the replay is not working.
    // If it panics for any other reason the test fails.
    assert_panic_contains(result, "foobar");
}
