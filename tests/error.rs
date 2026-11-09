use std::time::Duration;

#[test]
fn io_errors_convert() {
    let e: casket::Error = std::io::Error::new(std::io::ErrorKind::TimedOut, "nope").into();
    assert!(matches!(e, casket::Error::Io(_)));
    assert!(e.to_string().starts_with("io:"));
}

#[test]
fn protocol_errors_name_the_protocol() {
    let e = casket::Error::Protocol {
        protocol: "modbus",
        detail: "bad function code".into(),
    };
    assert_eq!(e.to_string(), "protocol modbus: bad function code");
}

#[test]
fn timeouts_carry_the_budget() {
    let e = casket::Error::Timeout(Duration::from_secs(3));
    assert_eq!(e.to_string(), "timeout after 3s");
}

#[test]
fn auth_errors_name_the_user() {
    let e = casket::Error::Auth { user: "admin".into() };
    assert_eq!(e.to_string(), "authentication failed for admin");
}

#[test]
fn unsupported_errors_are_self_describing() {
    let e = casket::Error::Unsupported("omron has no write path".into());
    assert_eq!(e.to_string(), "unsupported: omron has no write path");
}
