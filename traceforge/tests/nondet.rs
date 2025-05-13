use traceforge::Nondet;
use traceforge::{verify, Config};

#[test]
fn test_nondet() {
    verify(Config::builder().build(), || {
        let start = 4;
        let end = 6;
        let numbers = start..end;
        let n1 = numbers.nondet();

        assert!(n1 >= start);
        assert!(n1 < end);

        // Show that the type system allows us to use the same range to extract
        // another nondet value. This is possible because the Nondet<T> type
        // from Must takes a reference.
        let _n2 = numbers.nondet();
    });
}
