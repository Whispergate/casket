//! Compile and behaviour check for the two code blocks in `docs/errors.md`.
//!
//! The blocks are pasted from the doc, indented one level to sit inside their modules. Inside the
//! crate they resolve `crate::error`; in an integration test `crate` is the test binary, so
//! `mod error` below re-exports the real module and the same text resolves unchanged. Nothing here
//! re-implements the error type.

mod error {
    pub use casket::error::{Error, Result};
}

/// The block headed "Before" in `docs/errors.md`, unchanged.
mod before {
    use socket2::{Domain, Protocol, Socket, Type};
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
    use std::time::Duration;

    pub fn create_udp_broadcast_socket(bind_ip: &str, timeout_secs: u64) -> anyhow::Result<UdpSocket> {
        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_reuse_address(true)?;
        sock.set_broadcast(true)?;
        sock.set_read_timeout(Some(Duration::from_secs(timeout_secs.max(1))))?;

        let bind_addr: SocketAddr = SocketAddr::new(bind_ip.parse::<Ipv4Addr>()?.into(), 0);
        sock.bind(&bind_addr.into())?;

        Ok(UdpSocket::from(sock))
    }
}

/// The block headed "After" in `docs/errors.md`, unchanged.
mod after {
    use crate::error::Result;
    use socket2::{Domain, Protocol, Socket, Type};
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
    use std::time::Duration;

    pub fn create_udp_broadcast_socket(bind_ip: &str, timeout_secs: u64) -> Result<UdpSocket> {
        let ip: Ipv4Addr = bind_ip.parse().map_err(|e| crate::error::Error::Protocol {
            protocol: "network",
            detail: format!("bad bind address {bind_ip}: {e}"),
        })?;

        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_reuse_address(true)?;
        sock.set_broadcast(true)?;
        sock.set_read_timeout(Some(Duration::from_secs(timeout_secs.max(1))))?;

        let bind_addr: SocketAddr = SocketAddr::new(ip.into(), 0);
        sock.bind(&bind_addr.into())?;

        Ok(UdpSocket::from(sock))
    }
}

#[test]
fn before_block_is_not_matchable() {
    let err = before::create_udp_broadcast_socket("not-an-ip", 1)
        .expect_err("an unparseable bind address must not produce a socket");

    assert!(
        err.downcast_ref::<casket::Error>().is_none(),
        "the anyhow version cannot hand back a casket::Error"
    );
    assert!(
        !err.to_string().starts_with("protocol "),
        "the anyhow version has no variant vocabulary, display was {err}"
    );
}

#[test]
fn after_block_names_the_failure() {
    let err = after::create_udp_broadcast_socket("not-an-ip", 1)
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
fn after_block_still_binds_a_valid_address() {
    let sock = after::create_udp_broadcast_socket("127.0.0.1", 1)
        .expect("a loopback bind must succeed");
    assert_eq!(
        sock.local_addr().expect("bound socket has an address").ip().to_string(),
        "127.0.0.1"
    );
}
