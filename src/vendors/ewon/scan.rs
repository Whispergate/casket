use anyhow::Result;
use std::net::UdpSocket;
use std::time::Duration;

use crate::core::network::NetworkInterface;

/// eWON IPCONF discovery UDP destination port.
pub const IPCONF_DISCOVERY_PORT: u16 = 1507;
const RESPONSE_PORT: u16 = 1506;

const DISCOVERY_PACKETS: &[&str] = &[
    "4950434f4e4600000000000000000000000000000000000000000000000000000000000000000000",
    "4950434f4e460000000000000000000a000000000000000000000000000000000000000000000000",
];

/// An eWON device discovered over the IPCONF UDP protocol (network, MAC, serial, firmware).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EwonDevice {
    pub ip: Option<String>,
    pub netmask: Option<String>,
    pub mac: Option<String>,
    pub serial: Option<String>,
    pub product_code: Option<String>,
    pub firmware: Option<String>,
    pub identifier: Option<String>,
    pub response_type: String,
}

fn parse_device_info(data: &[u8]) -> EwonDevice {
    let to_hex = |b: u8| format!("{b:02x}");

    let ip = if data.len() >= 28 {
        Some(format!(
            "{}.{}.{}.{}",
            data[23], data[22], data[21], data[20]
        ))
    } else {
        None
    };

    let netmask = if data.len() >= 32 {
        Some(format!(
            "{}.{}.{}.{}",
            data[27], data[26], data[25], data[24]
        ))
    } else {
        None
    };

    let mac = if data.len() >= 38 {
        Some(format!(
            "{}:{}:{}:{}:{}:{}",
            to_hex(data[32]),
            to_hex(data[33]),
            to_hex(data[34]),
            to_hex(data[35]),
            to_hex(data[36]),
            to_hex(data[37])
        ))
    } else {
        None
    };

    let serial = if data.len() >= 20 {
        let serialp1 = data[19];
        let raw16 = if data.len() >= 19 {
            (u16::from(data[18]) << 8) | u16::from(data[17])
        } else {
            0
        };
        let serialp2 = raw16 / 1000;
        let mut serialp3 = data.get(17).copied().unwrap_or(0);
        if raw16 % 1000 >= 500 {
            serialp3 = serialp3.wrapping_add(0x80);
        }
        let serialp4 = data.get(16).copied().unwrap_or(0);
        Some(format!("{serialp1}{serialp2}-{serialp3:04}-{serialp4}"))
    } else {
        None
    };

    let product_code = data.get(16).map(std::string::ToString::to_string);
    let identifier = if data.len() >= 4 {
        Some(String::from_utf8_lossy(&data[..4]).to_string())
    } else {
        None
    };

    EwonDevice {
        ip,
        netmask,
        mac,
        serial,
        product_code,
        firmware: None,
        identifier,
        response_type: "device_info".into(),
    }
}

fn parse_firmware_info(data: &[u8]) -> EwonDevice {
    let firmware = if data.len() >= 20 {
        let raw = &data[20..];
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        Some(String::from_utf8_lossy(&raw[..end]).trim().to_string())
    } else {
        None
    };

    EwonDevice {
        ip: None,
        netmask: None,
        mac: None,
        serial: None,
        product_code: None,
        firmware,
        identifier: None,
        response_type: "firmware_info".into(),
    }
}

fn parse_response(data: &[u8]) -> Option<EwonDevice> {
    if data.len() < 16 {
        return None;
    }
    match data[15] {
        2 => Some(parse_device_info(data)),
        5 => Some(parse_firmware_info(data)),
        _ => None,
    }
}

/// Broadcast-scan for eWON devices on the selected interface.
pub fn scan(interface: &NetworkInterface, timeout: u64, silent: bool) -> Result<Vec<EwonDevice>> {
    scan_with_port(interface, timeout, silent, IPCONF_DISCOVERY_PORT)
}

/// Record one unique IPCONF reply.
///
/// IPCONF discovery deliberately sends two request variants.  Some devices
/// answer both with the same device-info payload, which must still render as
/// one discovered device.
fn record_unique_response(devices: &mut Vec<EwonDevice>, device: EwonDevice) -> bool {
    if devices.contains(&device) {
        false
    } else {
        devices.push(device);
        true
    }
}

/// Broadcast-scan for eWON devices using an explicit IPCONF UDP destination.
///
/// This is discovery-only. It sends the fixed IPCONF requests to the selected
/// interface's broadcast address and does not contact the web/API endpoint.
pub fn scan_with_port(
    interface: &NetworkInterface,
    timeout: u64,
    silent: bool,
    port: u16,
) -> Result<Vec<EwonDevice>> {
    use socket2::{Domain, Protocol, Socket, Type};

    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    s.set_broadcast(true)?;
    s.set_read_timeout(Some(Duration::from_secs(timeout)))?;

    let bind: std::net::SocketAddr = format!("{}:{RESPONSE_PORT}", interface.ip).parse()?;
    if s.bind(&bind.into()).is_err() {
        // Fall back to an ephemeral port on the selected interface if 1506 is in use.
        let fallback: std::net::SocketAddr = format!("{}:0", interface.ip).parse()?;
        s.bind(&fallback.into())?;
    }
    let sock = UdpSocket::from(s);

    if !silent {
        crate::casket_outln!("Sending discovery packets, waiting {timeout}s for answers...");
    }

    let discovery_port = if port == 0 { IPCONF_DISCOVERY_PORT } else { port };
    let broadcast = crate::core::network::calculate_broadcast(&interface.ip, &interface.netmask);
    for pkt_hex in DISCOVERY_PACKETS {
        let pkt = hex_decode(pkt_hex);
        let _ = sock.send_to(&pkt, format!("{broadcast}:{discovery_port}"));
    }

    let mut devices = Vec::new();
    let mut buf = [0u8; 1024];

    loop {
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                if let Some(dev) = parse_response(&buf[..n]) {
                    if record_unique_response(&mut devices, dev.clone()) && !silent {
                        print_device(&dev);
                    }
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(_) => break,
        }
    }

    if !silent {
        let device_count = devices.iter().filter(|d| d.response_type == "device_info").count();
        crate::casket_outln!("Got {device_count} device(s).");
    }
    Ok(devices)
}

/// Send eWON discovery to a specific IP.
pub fn scan_ip(ip: &str, timeout: u64, silent: bool) -> Result<Vec<EwonDevice>> {
    scan_ip_with_port(ip, timeout, silent, IPCONF_DISCOVERY_PORT)
}

/// Send eWON IPCONF discovery to a specific IP and UDP destination port.
///
/// This remains a read-only discovery exchange; it never probes the web/API port.
pub fn scan_ip_with_port(
    ip: &str,
    timeout: u64,
    silent: bool,
    port: u16,
) -> Result<Vec<EwonDevice>> {
    use socket2::{Domain, Protocol, Socket, Type};

    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_read_timeout(Some(Duration::from_secs(timeout)))?;

    let bind: std::net::SocketAddr = format!("0.0.0.0:{RESPONSE_PORT}").parse()?;
    if s.bind(&bind.into()).is_err() {
        let fallback: std::net::SocketAddr = "0.0.0.0:0".parse()?;
        s.bind(&fallback.into())?;
    }
    let sock = UdpSocket::from(s);

    let discovery_port = if port == 0 { IPCONF_DISCOVERY_PORT } else { port };
    for pkt_hex in DISCOVERY_PACKETS {
        let pkt = hex_decode(pkt_hex);
        let _ = sock.send_to(&pkt, format!("{ip}:{discovery_port}"));
    }

    let mut devices = Vec::new();
    let mut buf = [0u8; 1024];

    loop {
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                if let Some(dev) = parse_response(&buf[..n]) {
                    if record_unique_response(&mut devices, dev.clone()) && !silent {
                        print_device(&dev);
                    }
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(_) => break,
        }
    }

    if devices.is_empty() && !silent {
        crate::casket_outln!("No eWON response from {ip}");
    }
    Ok(devices)
}

/// Read live tag values from an eWON Flexy via the REST API (`GET /api/tags/`).
///
/// Returns a list of `(name, value_string)` pairs. Pass `port = 0` for default (80).
/// Pass `creds = Some(("username", "password"))` to authenticate via HTTP Basic Auth.
pub fn read_tag_values(ip: &str, port: u16, creds: Option<(&str, &str)>) -> Result<Vec<(String, String)>> {
    use base64::Engine as _;
    let effective_port = if port == 0 { 80 } else { port };
    let url = format!("http://{ip}:{effective_port}/api/tags/");
    let mut req = ureq::get(&url).timeout(Duration::from_secs(5));
    if let Some((user, pass)) = creds {
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
        req = req.set("Authorization", &format!("Basic {encoded}"));
    }
    let body = req.call()?.into_string()?;
    let v: serde_json::Value = serde_json::from_str(&body)?;
    let mut tags = Vec::new();
    if let Some(arr) = v.get("body").and_then(|b| b.as_array()) {
        for item in arr {
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("?").to_string();
            let value = item.get("value").map_or_else(String::new, |vv| {
                vv.as_str().map_or_else(|| vv.to_string(), str::to_string)
            });
            tags.push((name, value));
        }
    }
    Ok(tags)
}

fn print_device(dev: &EwonDevice) {
    if dev.response_type == "device_info" {
        crate::casket_outln!(
            "  - {}, {}, {}, {}, Pcode: {}",
            dev.identifier.as_deref().unwrap_or("?"),
            dev.ip.as_deref().unwrap_or("?"),
            dev.netmask.as_deref().unwrap_or("?"),
            dev.mac.as_deref().unwrap_or("?"),
            dev.product_code.as_deref().unwrap_or("?")
        );
    } else if let Some(fw) = &dev.firmware {
        crate::casket_outln!("    Firmware: {fw}");
    }
}

fn hex_decode(s: &str) -> Vec<u8> {
    if !s.len().is_multiple_of(2) { return vec![]; }
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decode_odd_length_returns_empty() {
        assert!(hex_decode("A").is_empty());
        assert!(hex_decode("ABC").is_empty());
    }

    #[test]
    fn hex_decode_even_length_works() {
        assert_eq!(hex_decode("DEAD"), vec![0xDE, 0xAD]);
    }

    #[test]
    fn parse_response_empty_returns_none() {
        assert!(parse_response(&[]).is_none());
    }

    #[test]
    fn parse_response_too_short_returns_none() {
        assert!(parse_response(&[0u8; 15]).is_none());
    }

    #[test]
    fn parse_response_type2_returns_device_info() {
        let mut data = vec![0u8; 16];
        data[15] = 2;
        let dev = parse_response(&data).unwrap();
        assert_eq!(dev.response_type, "device_info");
    }

    #[test]
    fn parse_response_type5_returns_firmware_info() {
        let mut data = vec![0u8; 16];
        data[15] = 5;
        let dev = parse_response(&data).unwrap();
        assert_eq!(dev.response_type, "firmware_info");
    }

    #[test]
    fn parse_response_unknown_type_returns_none() {
        let mut data = vec![0u8; 16];
        data[15] = 99;
        assert!(parse_response(&data).is_none());
    }

    #[test]
    fn parse_device_info_empty_has_no_fields() {
        let dev = parse_device_info(&[]);
        assert!(dev.ip.is_none());
        assert!(dev.mac.is_none());
        assert!(dev.serial.is_none());
        assert!(dev.identifier.is_none());
        assert_eq!(dev.response_type, "device_info");
    }

    #[test]
    fn parse_device_info_with_identifier() {
        let mut data = vec![0u8; 4];
        data[..4].copy_from_slice(b"IPCO");
        let dev = parse_device_info(&data);
        assert_eq!(dev.identifier.as_deref(), Some("IPCO"));
    }

    #[test]
    fn parse_firmware_info_empty_has_no_firmware() {
        let dev = parse_firmware_info(&[]);
        assert!(dev.firmware.is_none());
        assert_eq!(dev.response_type, "firmware_info");
    }

    #[test]
    fn parse_firmware_info_with_version() {
        let mut data = vec![0u8; 28];
        data[20..27].copy_from_slice(b"14.6s0\0");
        let dev = parse_firmware_info(&data);
        assert_eq!(dev.firmware.as_deref(), Some("14.6s0"));
    }

    #[test]
    fn repeated_ipconf_reply_is_listed_once() {
        let mut raw = vec![0u8; 38];
        raw[..4].copy_from_slice(b"IPCO");
        raw[15] = 2;
        raw[20..24].copy_from_slice(&[1, 0, 0, 127]);
        raw[32..38].copy_from_slice(&[0xde, 0xad, 0xbe, 0xef, 0x00, 0x01]);
        let device = parse_response(&raw).unwrap();
        let mut devices = Vec::new();
        assert!(record_unique_response(&mut devices, device.clone()));
        assert!(!record_unique_response(&mut devices, device));
        assert_eq!(devices.len(), 1);
    }
}
