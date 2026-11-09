//! Regression lock on the "nothing there" path: an address with no device must come back empty,
//! without panicking, and inside the caller's budget. Every scan starts by asking that question.
//!
//! Three details differ from the queue's draft, all checked against `src/core/autodetect.rs`:
//!
//! - `probe_all`/`sweep` are `fn(ip: &str, timeout_secs: u64)`. There is no port argument and no
//!   sub-second budget, so `127.0.0.1:<port>` and a 300 ms timeout cannot be expressed. Probes
//!   always target their compiled-in ports, and 1 s is the smallest budget the API accepts.
//! - Loopback is not "nothing there" on a developer box. On the machine this was written on, a DNP3
//!   simulator listens on `0.0.0.0:20000` and an MQTT broker on `127.0.0.1:1883`, so probing
//!   `127.0.0.1` or `127.0.0.2` correctly reports a DNP3 device. The silent address used here is
//!   `192.0.2.1` (TEST-NET-1, RFC 5737): reserved for documentation, it cannot host a device, so a
//!   connect to it can only time out or fail.
//! - The queue's bind/drop step is kept as its own test, since the ephemeral port can still be
//!   checked even though the probe cannot be aimed at it.

use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use casket::core::autodetect;

/// An address reserved for documentation, so there is no device behind it.
const SILENT_ADDR: &str = "192.0.2.1";

/// The API's floor: the budget is whole seconds, so 300 ms rounds up to this.
const SILENT_TIMEOUT_SECS: u64 = 1;

/// The queue's setup: bind an ephemeral loopback port, read it, drop the listener.
fn dropped_ephemeral_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
    let port = listener.local_addr().expect("bound listener has an address").port();
    drop(listener);
    port
}

#[test]
fn a_dropped_ephemeral_port_accepts_nothing() {
    let port = dropped_ephemeral_port();
    let addr = format!("127.0.0.1:{port}").parse().expect("literal address parses");
    let attempt = TcpStream::connect_timeout(&addr, Duration::from_millis(300));

    assert!(
        attempt.is_err(),
        "port {port} still accepts connections after the listener was dropped"
    );
}

#[test]
fn probe_all_finds_nothing_on_a_silent_address() {
    let started = Instant::now();
    let devices = autodetect::probe_all(SILENT_ADDR, SILENT_TIMEOUT_SECS);
    let elapsed = started.elapsed();

    let vendors: Vec<&str> = devices.iter().map(|device| device.vendor.as_str()).collect();
    assert!(devices.is_empty(), "expected no devices, got {vendors:?}");
    assert!(
        elapsed < Duration::from_secs(2),
        "probe_all took {elapsed:?} against a silent address, the budget is 2s"
    );
}

#[test]
fn sweep_reports_every_probe_as_empty() {
    let outcomes = autodetect::sweep(SILENT_ADDR, SILENT_TIMEOUT_SECS);

    let answering: Vec<&str> = outcomes
        .iter()
        .filter(|outcome| outcome.device.is_some())
        .map(|outcome| outcome.probe.label)
        .collect();
    assert!(answering.is_empty(), "expected silence, these probes answered: {answering:?}");
    assert_eq!(
        outcomes.len(),
        autodetect::builtin_protocols().len(),
        "an unresponsive probe must still produce a row"
    );
}
