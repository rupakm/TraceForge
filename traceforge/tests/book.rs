// Book examples

use traceforge::*;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Work,
    Terminate,
}

fn intro_race_model() {
    let t1 = thread::spawn(move || {
        let mut ready = true;
        let mut i = 0;
        loop {
            let m = traceforge::recv_msg_block();
            match m {
                Msg::Work => traceforge::assert(ready),
                Msg::Terminate => ready = false,
            }
            i = i + 1;
            if i == 2 {
                break;
            }
        }
    });
    let t1_id = t1.thread().id();

    let t2 = thread::spawn(move || {
        traceforge::send_msg(t1_id, Msg::Work);
    });
    traceforge::send_msg(t1_id.clone(), Msg::Terminate);

    let _ = t1.join();
    let _ = t2.join();
}

#[test]
fn intro_race_check() {
    let stats = traceforge::verify(
        Config::builder().with_keep_going_after_error(true).build(),
        intro_race_model,
    );
    println!("Stats = {}, {}", stats.execs, stats.block);
}
