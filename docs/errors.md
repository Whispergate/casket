# Errors

`casket` has one error type, `casket::Error` (`src/error.rs`), and every protocol returns
`casket::Result<T>`. The point is that a caller can match on the failure instead of parsing a string,
and that a report can render any failure without knowing which protocol produced it.

```rust
pub enum Error {
    Io(std::io::Error),
    Timeout(Duration),
    Protocol { protocol: &'static str, detail: String },
    Auth { user: String },
    Unsupported(String),
}
```

## Which variant to use when

| What happened | Variant | Display | Example detail |
|---|---|---|---|
| Socket died, was refused, or could not be opened | `Io` | `io: <os error>` | from `?` on any `std::io::Error` |
| Peer stayed silent for the whole budget | `Timeout` | `timeout after 3s` | the budget the caller set |
| Peer answered, and the answer is wrong for this protocol | `Protocol` | `protocol modbus: bad function code` | name the field that did not parse |
| Peer rejected the credentials | `Auth` | `authentication failed for admin` | the account refused |
| The protocol has no such operation, today | `Unsupported` | `unsupported: modbus write on unit 0` | the verb and the target |

Rules of thumb:

- `Io` means the transport failed. `Timeout` means the transport worked and the peer did not answer.
  Keep them apart: a scan report treats "closed port" and "silent device" differently.
- `Protocol` is for a peer that answered and whose answer we could not believe: short frame, bad CRC,
  a function code the spec does not define, a string where a number belongs. Always set `protocol` to
  the slug of the module (`modbus`, `s7`, `enip`, ...), never `"casket"` or `"unknown"`.
- `Auth` is only for credentials the peer actively refused. A credential that was never sent because
  the connection timed out is a `Timeout`.
- `Unsupported` means the operation exists in the trait vocabulary but not for this protocol. If the
  operation is simply not written yet, that is also `Unsupported`, and the detail should say so, for
  example `unsupported: dnp3 write, read only for now`.

## Migration recipe

This is the recipe Task 03 proved on `src/core/network.rs`, and the one to use for every module still
on `anyhow`:

1. `grep -n 'anyhow' <file>` to size the surface before touching it.
2. Replace `anyhow::Result<T>` with `crate::error::Result<T>` in the signature.
3. Replace `anyhow::bail!("x")` with
   `return Err(crate::error::Error::Protocol { protocol: "<slug>", detail: "x".into() });`
4. Replace `.context("x")?` with
   `.map_err(|e| crate::error::Error::Protocol { protocol: "<slug>", detail: format!("x: {e}") })?`
   when the failure is the peer's fault. If it is an OS failure, leave it as `?` so `#[from]` makes it
   `Io`.
5. Leave `// Errors use crate::error; see docs/errors.md.` at the top of the file.

The `?` operator still does most of the work: `From<std::io::Error>` is implemented, so a plain `?` on
a socket call becomes `Error::Io` with no edit. Only the cases that used to become opaque
`anyhow::Error` need a `map_err`.

### Before

```rust
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
```

### After

```rust
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
```

Three things changed and nothing else:

- the return type, so callers get `casket::Error` back through `?`;
- the bind-address parse, which is a peer-input failure, so it becomes `Protocol` with the protocol
  slug and the offending string in the detail;
- the socket calls keep a bare `?`, because `Io` already converts.

## Checking these examples

Both blocks above are pasted into `tests/errors_doc.rs` and compiled there (indented one level, since
each lives in its own module), so a doc edit that stops matching the crate fails
`cargo test -p casket`. Inside the crate the paths read `crate::error`; the test file provides a
`mod error` alias so the same text resolves in an integration test. If you change the example, change
the test.
