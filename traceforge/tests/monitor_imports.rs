// This test ensures that you can define monitors without being forced to import all of the
// structs and traits that monitors depend on.
// The rest of the file is written with no imports so that the monitor macro expansion
// cannot accidentally depend on any import.
// In the future, if this fails to compile, this is a sign that the definition of the monitor
// macro should be changed to explicitly qualify all the names it depends on.
use traceforge_macros::monitor;

#[monitor()]
#[derive(Clone, Debug, Default)]
pub struct MyMonitor {}

impl traceforge::monitor_types::Monitor for MyMonitor {}

#[test]
pub fn test_monitor() {
    traceforge::verify(traceforge::ConfigBuilder::new().build(), || {
        let _: ::traceforge::thread::JoinHandle<traceforge::monitor_types::MonitorResult> =
            start_monitor_my_monitor(MyMonitor {});
    });
}
