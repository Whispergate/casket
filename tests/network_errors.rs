//! The first migrated module (`src/core/network.rs`) must speak `casket::Error`, not `anyhow::Error`.

#[test]
fn bad_bind_address_is_a_protocol_error() {
    let err = casket::core::network::create_udp_broadcast_socket("not-an-ip", 1)
        .expect_err("an unparseable bind address must not produce a socket");

    match &err {
        casket::Error::Protocol { protocol, detail } => {
            assert_eq!(*protocol, "network");
            assert!(detail.contains("not-an-ip"), "detail was {detail}");
        }
        other => panic!("expected Error::Protocol, got {other:?}"),
    }
    assert!(
        err.to_string().starts_with("protocol network:"),
        "display was {err}"
    );
}

#[test]
fn valid_bind_address_still_returns_a_socket() {
    let sock = casket::core::network::create_udp_broadcast_socket("127.0.0.1", 1)
        .expect("a loopback bind must succeed");
    assert_eq!(
        sock.local_addr().expect("bound socket has an address").ip().to_string(),
        "127.0.0.1"
    );
}
