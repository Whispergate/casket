//! `DeviceClient` seen from outside the crate: a caller defines its own connected handle, stores a
//! `DeviceInfo` in it, and hands it to generic code that knows only the trait.
//!
//! The two handles below are test-local: a caller outside the crate defines them, which is the
//! public-surface half of the proof. The last implementor is the crate's own IEC 104 session, built
//! by its real `connect` against a loopback outstation, which is the other half.

use casket::core::autodetect::DeviceInfo;
use casket::core::client::DeviceClient;
use casket::error::Result;
use casket::vendors::iec104::client::{connect, Iec104Session};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;

struct FakeHandle {
    info: DeviceInfo,
    closes: u32,
}

impl FakeHandle {
    fn new(vendor: &str, ip: &str) -> Self {
        Self {
            info: DeviceInfo {
                vendor: vendor.into(),
                ip: ip.into(),
                fields: HashMap::new(),
            },
            closes: 0,
        }
    }
}

impl DeviceClient for FakeHandle {
    fn device_info(&self) -> &DeviceInfo {
        &self.info
    }

    fn close(&mut self) -> Result<()> {
        self.closes += 1;
        Ok(())
    }
}

struct OtherHandle {
    info: DeviceInfo,
}

impl DeviceClient for OtherHandle {
    fn device_info(&self) -> &DeviceInfo {
        &self.info
    }

    fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

fn describe<D: DeviceClient>(device: &D) -> String {
    format!("{} at {}", device.device_info().vendor, device.device_info().ip)
}

fn bound<T: DeviceClient>() {}

#[test]
fn an_external_type_can_implement_the_trait() {
    bound::<FakeHandle>();
    let handle = FakeHandle::new("modbus", "192.0.2.20");
    assert_eq!(handle.device_info().vendor, "modbus");
    assert_eq!(handle.device_info().ip, "192.0.2.20");
}

#[test]
fn generic_code_takes_any_implementor() {
    let first = FakeHandle::new("modbus", "192.0.2.20");
    let second = OtherHandle {
        info: DeviceInfo {
            vendor: "siemens".into(),
            ip: "192.0.2.21".into(),
            fields: HashMap::new(),
        },
    };
    assert_eq!(describe(&first), "modbus at 192.0.2.20");
    assert_eq!(describe(&second), "siemens at 192.0.2.21");
}

#[test]
fn close_can_be_called_twice() {
    let mut handle = FakeHandle::new("modbus", "192.0.2.20");
    assert!(handle.close().is_ok());
    assert!(handle.close().is_ok());
    assert_eq!(handle.closes, 2);
}

// ─── a real vendor implementor ────────────────────────────────────────────────

// The handles above are test-local. The session below is the crate's own IEC 104 client, aimed at a
// loopback listener that answers STARTDT, so the record checked here is built by the real connect
// path rather than by this file.

/// Answer one IEC 104 STARTDT act with STARTDT con, then close.
fn fake_outstation() -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().expect("local addr").port();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        let mut request = [0u8; 6];
        socket.read_exact(&mut request).expect("read STARTDT act");
        assert_eq!(request, [0x68, 0x04, 0x07, 0x00, 0x00, 0x00]);
        socket
            .write_all(&[0x68, 0x04, 0x0B, 0x00, 0x00, 0x00])
            .expect("write STARTDT con");
    });
    (port, server)
}

#[test]
fn an_iec104_session_is_a_device_client() {
    bound::<Iec104Session>();
}

#[test]
fn iec104_session_reports_what_connect_learned() {
    let (port, server) = fake_outstation();
    let mut session = connect("127.0.0.1", port).expect("connect to fake outstation");

    assert_eq!(session.device_info().vendor, "iec104");
    assert_eq!(session.device_info().ip, "127.0.0.1");
    let port_field = session.device_info().fields.get("port").expect("port recorded");
    assert_eq!(port_field.as_u64(), Some(u64::from(port)));
    let cap_field = session
        .device_info()
        .fields
        .get("cap_iec104_tcp")
        .expect("capability flag recorded");
    assert_eq!(
        cap_field.as_bool(),
        Some(true),
        "STARTDT was confirmed, so the TCP data transfer phase is a fact"
    );

    assert!(session.close().is_ok());
    assert!(session.close().is_ok());
    // Closing the socket does not erase what was learned before it closed.
    assert_eq!(session.device_info().ip, "127.0.0.1");
    server.join().expect("server thread");
}
