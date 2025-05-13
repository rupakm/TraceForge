use log::LevelFilter;
use simplelog::{CombinedLogger, SimpleLogger};
use std::any::Any;
use std::sync::Once;

static INIT_LOG: Once = Once::new();

#[allow(dead_code)] // Only used in tests
pub fn init_log() {
    INIT_LOG.call_once(|| {
        CombinedLogger::init(vec![SimpleLogger::new(
            LevelFilter::Trace,
            simplelog::Config::default(),
        )])
        .unwrap()
    });
}

#[allow(dead_code)] // Only used in tests
pub fn assert_panic_msg(result: Result<(), Box<dyn Any + Send>>, expected_msg: &str) {
    match result {
        Ok(_) => {
            panic!("The function was expected to panic, but it did not.");
        }
        Err(msg) => {
            if let Some(s) = msg.downcast_ref::<String>() {
                assert_eq!(s, expected_msg);
            } else if let Some(&s) = msg.downcast_ref::<&str>() {
                assert_eq!(s, expected_msg);
            } else {
                // See https://users.rust-lang.org/t/return-value-from-catch-unwind-is-a-useless-any/89134/4
                panic!("The panic did not return a string; can't display it.");
            }
        }
    }
}

#[allow(dead_code)] // Only used in tests
pub fn assert_panic_contains(result: Result<(), Box<dyn Any + Send>>, expected_msg: &str) {
    match result {
        Ok(_) => {
            panic!("The function was expected to panic, but it did not.");
        }
        Err(msg) => {
            if let Some(s) = msg.downcast_ref::<String>() {
                assert!(
                    s.contains(expected_msg),
                    "Expected `{}` in `{}`",
                    expected_msg,
                    s
                );
            } else if let Some(&s) = msg.downcast_ref::<&str>() {
                assert!(
                    s.contains(expected_msg),
                    "Expected `{}` in `{}`",
                    expected_msg,
                    s
                );
            } else {
                // See https://users.rust-lang.org/t/return-value-from-catch-unwind-is-a-useless-any/89134/4
                panic!("The panic did not return a string; can't display it.");
            }
        }
    }
}
