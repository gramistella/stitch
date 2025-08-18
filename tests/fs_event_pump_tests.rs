// tests/fs_event_pump_tests.rs  (new)

use stitch::core::drain_channel_nonblocking;

#[test]
fn drain_channel_nonblocking_reports_and_drains() {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel::<i32>();

    // Empty channel → no events
    assert!(!drain_channel_nonblocking(&rx));

    // Send a couple of items → returns true and drains them
    tx.send(1).unwrap();
    tx.send(2).unwrap();
    assert!(drain_channel_nonblocking(&rx));

    // Now empty again
    assert!(!drain_channel_nonblocking(&rx));

    // Disconnected should not panic and should return false
    drop(tx);
    assert!(!drain_channel_nonblocking(&rx));
}
