//! Device autodetection: probe a single IP or sweep a range across all supported vendors.

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

static STEALTH_MODE: AtomicBool = AtomicBool::new(false);

/// Enable or disable stealth scanning: randomised probe order + inter-probe jitter.
pub fn set_stealth(enabled: bool) {
    STEALTH_MODE.store(enabled, Ordering::Relaxed);
}

/// Returns true when stealth mode is active.
pub fn stealth_enabled() -> bool {
    STEALTH_MODE.load(Ordering::Relaxed)
}

/// One successful probe: which vendor answered, at which address, and what it said.
///
/// Produced by [`probe_builtin`], [`sweep`], or [`VendorRegistry::probe_all`]. The vendor-specific
/// part is flattened into [`fields`](DeviceInfo::fields) instead of a typed struct, so one sweep can
/// return records from every protocol without the caller matching on each one.
///
/// Treat identity values as a lead, not as proof: they come from whatever the probe could read, and
/// a device can lie or be misidentified. Confirm before acting on one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// The probe slug that matched, e.g. `"beckhoff"` or `"siemens"`.
    ///
    /// Chosen by the probe, not read from the device. Some slugs name a protocol family rather than a
    /// make (`"modicon"`, `"enip"`), so a hit does not pin down the hardware.
    pub vendor: String,

    /// The address that was probed, copied from the caller's argument.
    ///
    /// Never read from the device. In a sweep this is the host that answered; in a direct probe it is
    /// the string that was passed in, so a hostname stays a hostname.
    pub ip: String,

    /// Vendor-specific facts, flattened: identity strings, ports, and capability flags.
    ///
    /// Keys vary by vendor, and the same key can carry different provenance within one vendor.
    /// `name`, `firmware`, `product_name`, `vendor_id` and `serial` are device-declared. Numeric
    /// `*port` keys are a mix of compiled-in constants from the probe and ports the scan actually
    /// found, so a port here means "the protocol this probe speaks", not proof that the service is
    /// reachable. `cap_*` entries are booleans, always present for that vendor, and may be false,
    /// which is how a negative answer is recorded. Other keys are omitted when the device did not
    /// report them, so an absent key means "not learned", never "false".
    #[serde(flatten)]
    pub fields: HashMap<String, serde_json::Value>,
}

/// Implemented by vendor device structs to convert themselves into the unified [`DeviceInfo`] type.
/// Use `#[derive(casket_macros::IntoDeviceInfo)]` (with the `macros` feature) to generate this
/// automatically from struct field annotations.
pub trait IntoDeviceInfo {
    /// The short vendor slug used as `DeviceInfo::vendor` (e.g. `"beckhoff"`, `"siemens"`).
    const VENDOR_SLUG: &'static str;
    /// Consume the vendor struct and produce a [`DeviceInfo`].
    fn into_device_info(self) -> DeviceInfo;
}

/// Vendor detection priority: lower number = higher confidence.
fn vendor_priority(vendor: &str) -> u8 {
    match vendor {
        "modicon" => 0,
        "beckhoff" | "siemens" | "rockwell" | "ewon" | "mitsubishi" | "schneider" | "phoenix"
        | "omron" | "bacnet" | "dnp3" | "opcua" => 1,
        "enip" | "iec104" => 2,
        "snmp" | "mqtt" => 3,
        _ => 99,
    }
}

/// Display metadata for one probe in a sweep: what the sweep calls it and where it looks.
#[derive(Clone, Copy)]
pub struct ProbeInfo {
    /// Human-readable protocol name for sweep output, e.g. `"Beckhoff ADS"`.
    pub label: &'static str,
    /// Transport and port description for sweep output, e.g. `"UDP 48899 / TCP 48898"`.
    pub transport: &'static str,
}

/// Metadata for one built-in protocol exposed by Casket.
///
/// Consumers can use this catalog to present exactly the protocols implemented by
/// Casket without maintaining a second, easily-stale list.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ProtocolInfo {
    /// Stable protocol selector used with [`probe_builtin`].
    pub slug: &'static str,
    /// Human-readable protocol name.
    pub label: &'static str,
    /// Transport and default port(s) used by the probe.
    pub transport: &'static str,
}

/// One protocol's result from `sweep`: what was probed and what (if anything) responded.
pub struct SweepOutcome {
    /// The probe that ran, even when it found nothing.
    pub probe: ProbeInfo,
    /// The record the probe returned, or `None` when nothing answered.
    pub device: Option<DeviceInfo>,
}

/// A single vendor probe entry: slug, display metadata, and detection function.
///
/// Pass to [`VendorRegistry::register`] to add a custom vendor alongside the built-ins.
#[derive(Clone, Copy)]
pub struct VendorProbe {
    /// Short identifier stored in [`DeviceInfo::vendor`] (e.g. `"beckhoff"`).
    pub slug: &'static str,
    /// Human-readable name shown in sweep output (e.g. `"Beckhoff ADS"`).
    pub label: &'static str,
    /// Transport description shown in sweep output (e.g. `"UDP 48899 / TCP 48898"`).
    pub transport: &'static str,
    /// Detection function: returns `Some(DeviceInfo)` on success, `None` on no match.
    pub probe: fn(&str) -> Option<DeviceInfo>,
}

fn probe_beckhoff(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::beckhoff::scan;
    let devices = scan::discover_ip(ip, 2, true).ok()?;
    let d = devices.into_iter().next()?;
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("name".into(), d.name.into());
    fields.insert("netid".into(), d.netid_str.into());
    fields.insert("tc_version".into(), d.tc_version.into());
    fields.insert("kernel".into(), d.kernel.into());
    fields.insert("port".into(), 48898i64.into());
    fields.insert("ads_port".into(), 48898i64.into());
    fields.insert(
        "web_port".into(),
        i64::from(crate::vendors::beckhoff::webcontrol::DEFAULT_WEB_PORT).into(),
    );
    fields.insert("discovery_port".into(), 48899i64.into());
    fields.insert("discovery_transport".into(), "udp".into());
    fields.insert("protocol".into(), "ads_tcp".into());
    fields.insert("cap_ads_tcp".into(), true.into());
    fields.insert("cap_ads_udp_discovery".into(), true.into());
    fields.insert("cap_beckhoff_web_candidate".into(), true.into());
    Some(DeviceInfo {
        vendor: "beckhoff".into(),
        ip: ip.into(),
        fields,
    })
}

fn probe_siemens(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::siemens::s7comm;
    use std::net::TcpStream;

    // Budget: 3s TCP probe + 2s per S7Comm read ≤ 7s total, within detect_device's 8s window.
    // tcp_scan uses a 1-second timeout that may miss WAN targets; probe directly here.
    TcpStream::connect_timeout(&format!("{ip}:102").parse().ok()?, Duration::from_secs(3)).ok()?;

    // S7-1500 with S7comm+ or access protection: setup_connection succeeds on the first
    // TSAP (c2020101) but SZL reads may return "Unknown": still identify the device.
    let (hardware, firmware, cpu_state) = s7comm::get_device_snapshot(ip, 102, 2);

    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(hw) = hardware {
        fields.insert("hardware".into(), hw.into());
    }
    if let Some(fw) = firmware {
        fields.insert("firmware".into(), fw.into());
    }
    fields.insert("cpu_state".into(), cpu_state.into());
    fields.insert("port".into(), 102i64.into());
    fields.insert("s7_port".into(), 102i64.into());
    fields.insert("open_ports".into(), vec![102i64].into());
    fields.insert("cap_s7_tcp".into(), true.into());
    Some(DeviceInfo {
        vendor: "siemens".into(),
        ip: ip.into(),
        fields,
    })
}

fn probe_enip(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::enip::scan;
    let devices = scan::scan_ip(ip, 2, true).ok()?;
    let d = devices.into_iter().next()?;

    let vendor_id = u32::from_str_radix(&d.vendor_id, 16).unwrap_or(0);
    let vendor_name = if [1u32, 5, 77].contains(&vendor_id) {
        "rockwell"
    } else {
        "enip"
    };

    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("product_name".into(), d.product_name.into());
    fields.insert("vendor_id".into(), d.vendor_id.into());
    fields.insert("device_type".into(), d.device_type.into());
    fields.insert("revision".into(), d.revision.into());
    fields.insert("port".into(), 44818i64.into());
    fields.insert("enip_port".into(), 44818i64.into());
    fields.insert("cap_enip_tcp_identity".into(), true.into());
    Some(DeviceInfo {
        vendor: vendor_name.into(),
        ip: ip.into(),
        fields,
    })
}

fn probe_ewon(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::ewon::scan;
    let devices = scan::scan_ip(ip, 2, true).ok()?;
    let d = devices.into_iter().next()?;
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(mac) = d.mac {
        fields.insert("mac".into(), mac.into());
    }
    if let Some(serial) = d.serial {
        fields.insert("serial".into(), serial.into());
    }
    fields.insert("http_port".into(), 80i64.into());
    fields.insert("cap_ewon_ipconf".into(), true.into());
    fields.insert("cap_ewon_http_candidate".into(), true.into());
    Some(DeviceInfo {
        vendor: "ewon".into(),
        ip: ip.into(),
        fields,
    })
}

fn probe_mitsubishi(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::mitsubishi::scan;
    let devices = scan::scan_ip(ip, 2, true).ok()?;
    let d = devices.into_iter().next()?;
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    let protocol = d.protocol.clone().unwrap_or_default();
    let discovery_transport = d.discovery_transport.clone().unwrap_or_default();
    fields.insert("plc_type".into(), d.plc_type.into());
    if let Some(title) = d.title {
        fields.insert("title".into(), title.into());
    }
    if let Some(comment) = d.comment {
        fields.insert("comment".into(), comment.into());
    }
    if let Some(protocol) = d.protocol {
        fields.insert("protocol".into(), protocol.into());
    }
    if let Some(port) = d.port {
        if matches!(d.discovery_transport.as_deref(), Some("tcp")) {
            fields.insert("port".into(), i64::from(port).into());
            fields.insert("slmp_port".into(), i64::from(port).into());
        } else {
            fields.insert("port".into(), 5007i64.into());
            fields.insert("discovery_port".into(), i64::from(port).into());
            fields.insert("slmp_port".into(), 5007i64.into());
        }
    } else {
        fields.insert("port".into(), 5007i64.into());
        fields.insert("slmp_port".into(), 5007i64.into());
    }
    if let Some(transport) = d.discovery_transport {
        fields.insert("discovery_transport".into(), transport.into());
    }
    fields.insert("cap_mitsubishi_identity".into(), true.into());
    fields.insert(
        "cap_gxworks_udp".into(),
        (protocol == "gxworks_udp" || discovery_transport == "udp").into(),
    );
    fields.insert(
        "cap_slmp_tcp".into(),
        (protocol == "slmp_tcp" || discovery_transport == "tcp").into(),
    );
    fields.insert("cap_slmp_udp".into(), (protocol == "slmp_udp").into());
    Some(DeviceInfo {
        vendor: "mitsubishi".into(),
        ip: ip.into(),
        fields,
    })
}

fn probe_schneider(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::schneider::scan;
    let devices = scan::scan_ip(ip, 3, true).ok()?;
    let d = devices.into_iter().next()?;
    if !d.identity_match {
        return None;
    }
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    let name_for_family = d.name.clone().unwrap_or_default();
    let identity_confirmed = d.identity_match
        || (matches!(d.discovery_transport.as_deref(), Some("udp"))
            && (d.name.is_some() || d.firmware.is_some()));
    let modbus_tcp_confirmed = d.protocol.as_deref() == Some("modbus_tcp") || d.port.is_some();
    let udp_confirmed =
        matches!(d.discovery_transport.as_deref(), Some("udp")) && identity_confirmed;
    if let Some(name) = d.name {
        fields.insert("name".into(), name.into());
    }
    if let Some(fw) = d.firmware {
        fields.insert("firmware".into(), fw.into());
    }
    if let Some(protocol) = d.protocol {
        fields.insert("protocol".into(), protocol.into());
    }
    if let Some(port) = d.port {
        fields.insert("port".into(), i64::from(port).into());
        fields.insert("modbus_port".into(), i64::from(port).into());
    } else {
        fields.insert("modbus_port".into(), 502i64.into());
    }
    if let Some(transport) = d.discovery_transport {
        fields.insert("discovery_transport".into(), transport.into());
    }
    if let Some(unit_id) = d.modbus_unit_id {
        fields.insert("modbus_unit_id".into(), i64::from(unit_id).into());
    }
    if identity_confirmed {
        fields.insert("web_port".into(), 80i64.into());
    }
    fields.insert("cap_identity_confirmed".into(), identity_confirmed.into());
    fields.insert("cap_schneider_udp".into(), udp_confirmed.into());
    fields.insert("cap_modbus_tcp".into(), modbus_tcp_confirmed.into());
    fields.insert(
        "fc90_family".into(),
        schneider_fc90_family(&name_for_family).into(),
    );
    Some(DeviceInfo {
        vendor: "schneider".into(),
        ip: ip.into(),
        fields,
    })
}

fn schneider_fc90_family(name: &str) -> &'static str {
    let name = name.to_ascii_lowercase();
    if name.contains("tm221") {
        "tm221"
    } else if name.contains("m340")
        || name.contains("quantum")
        || name.contains("premium")
        || name.contains("m580")
    {
        "m340_quantum_premium"
    } else {
        "unknown"
    }
}

fn probe_phoenix(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::phoenix::control;
    let info = control::get_device_info(ip, 0, true).ok()?;
    if info.plc_type.is_empty() {
        return None;
    }
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("plc_type".into(), info.plc_type.into());
    if let Some(fw) = info.firmware {
        fields.insert("firmware".into(), fw.into());
    }
    fields.insert("port".into(), 1962i64.into());
    fields.insert("phoenix_info_port".into(), 1962i64.into());
    fields.insert("webvisit_port".into(), 80i64.into());
    fields.insert("cap_phoenix_info".into(), true.into());
    fields.insert("cap_webvisit_candidate".into(), true.into());
    Some(DeviceInfo {
        vendor: "phoenix".into(),
        ip: ip.into(),
        fields,
    })
}

fn probe_omron(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::omron::{fins, scan};
    let (dev, tcp_confirmed) = match fins::get_device_info_tcp(ip, 0) {
        Ok(dev) => (dev, true),
        Err(_) => (scan::scan_ip(ip)?, false),
    };
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("model".into(), dev.model.into());
    fields.insert("version".into(), dev.version.into());
    fields.insert(
        "node".into(),
        serde_json::Value::Number(dev.node_addr.into()),
    );
    fields.insert("port".into(), 9600i64.into());
    fields.insert("fins_port".into(), 9600i64.into());
    fields.insert("cap_fins_tcp".into(), tcp_confirmed.into());
    fields.insert("cap_fins_udp".into(), (!tcp_confirmed).into());
    fields.insert(
        "discovery_transport".into(),
        if tcp_confirmed { "tcp" } else { "udp" }.into(),
    );
    Some(DeviceInfo {
        vendor: "omron".into(),
        ip: ip.into(),
        fields,
    })
}

fn probe_iec104(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::iec104::client;
    if !client::probe(ip, 0) {
        return None;
    }
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert(
        "port".into(),
        serde_json::Value::Number(client::IEC104_PORT.into()),
    );
    fields.insert(
        "iec104_port".into(),
        serde_json::Value::Number(client::IEC104_PORT.into()),
    );
    fields.insert("cap_iec104_tcp".into(), true.into());
    Some(DeviceInfo {
        vendor: "iec104".into(),
        ip: ip.into(),
        fields,
    })
}

fn probe_snmp(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::snmp::{client, oids};

    // Try common community strings; 4-second UDP timeout per attempt
    let community = client::discover_community(ip, 0)?;

    // Fetch the four most useful scalar OIDs in one round-trip
    let sys_oids = [oids::SYS_DESCR, oids::SYS_OBJECT_ID, oids::SYS_NAME, oids::SYS_LOCATION];
    let results = client::get_multi(ip, 0, &community, &sys_oids).ok()?;
    let at = |i: usize| results.get(i).map(|(_, v)| v.display()).unwrap_or_default();

    let sys_descr = at(0);
    let sys_oid = at(1);
    let sys_name = at(2);
    let sys_location = at(3);

    let is_apc = sys_oid.starts_with(oids::APC_ROOT);

    // If sysObjectID maps to a known ICS vendor, return that vendor slug so the
    // device appears in the correct exploit list; otherwise fall through as "snmp".
    let vendor = oids::vendor_from_sys_oid(&sys_oid).unwrap_or("snmp");

    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("snmp_community".into(), community.clone().into());
    fields.insert("sys_descr".into(), sys_descr.into());
    fields.insert("sys_object_id".into(), sys_oid.into());
    fields.insert("sys_name".into(), sys_name.into());
    fields.insert("sys_location".into(), sys_location.into());
    fields.insert("port".into(), serde_json::Value::Number(client::SNMP_PORT.into()));
    fields.insert("snmp_port".into(), serde_json::Value::Number(client::SNMP_PORT.into()));
    fields.insert("cap_snmp_udp".into(), true.into());
    if is_apc {
        fields.insert("cap_snmp_apc".into(), true.into());
    }

    Some(DeviceInfo { vendor: vendor.into(), ip: ip.into(), fields })
}

fn probe_mqtt(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::mqtt::client;
    let d = client::probe(ip, client::MQTT_PORT)?;
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("port".into(), i64::from(d.port).into());
    fields.insert("mqtt_port".into(), i64::from(d.port).into());
    fields.insert("anonymous".into(), d.anonymous.into());
    fields.insert("sparkplug".into(), d.sparkplug.into());
    if let Some(info) = d.broker_info {
        fields.insert("broker_info".into(), info.into());
    }
    fields.insert("cap_mqtt_tcp".into(), true.into());
    Some(DeviceInfo { vendor: "mqtt".into(), ip: ip.into(), fields })
}

fn probe_bacnet(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::bacnet::client;
    let dev = client::scan_ip(ip, 3)?;
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("port".into(), i64::from(client::BACNET_PORT).into());
    fields.insert("bacnet_port".into(), i64::from(client::BACNET_PORT).into());
    fields.insert("instance_id".into(), i64::from(dev.instance_id).into());
    fields.insert("vendor_id".into(), i64::from(dev.vendor_id).into());
    if !dev.object_name.is_empty() {
        fields.insert("object_name".into(), dev.object_name.into());
    }
    if !dev.vendor_name.is_empty() {
        fields.insert("vendor_name".into(), dev.vendor_name.into());
    }
    if !dev.description.is_empty() {
        fields.insert("description".into(), dev.description.into());
    }
    if !dev.firmware_revision.is_empty() {
        fields.insert("firmware".into(), dev.firmware_revision.into());
    }
    fields.insert("cap_bacnet_udp".into(), true.into());
    Some(DeviceInfo { vendor: "bacnet".into(), ip: ip.into(), fields })
}

fn probe_dnp3(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::dnp3::client;
    let dev = client::detect(ip, Duration::from_secs(3))?;
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("port".into(), i64::from(client::DNP3_PORT).into());
    fields.insert("dnp3_port".into(), i64::from(client::DNP3_PORT).into());
    fields.insert("outstation_addr".into(), i64::from(dev.outstation_addr).into());
    fields.insert("cap_dnp3_tcp".into(), true.into());
    Some(DeviceInfo { vendor: "dnp3".into(), ip: ip.into(), fields })
}

fn probe_opcua(ip: &str) -> Option<DeviceInfo> {
    use crate::vendors::opcua::client;
    let server = client::detect(ip, 0, Duration::from_secs(3))?;
    let endpoints = client::get_endpoints(ip, 0, Duration::from_secs(5));
    let anonymous_count = endpoints.iter().filter(|e| e.allows_anonymous).count();
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("port".into(), i64::from(server.port).into());
    fields.insert("opcua_port".into(), i64::from(server.port).into());
    fields.insert("endpoint_count".into(), i64::try_from(endpoints.len()).unwrap_or(i64::MAX).into());
    fields.insert("anonymous_endpoints".into(), i64::try_from(anonymous_count).unwrap_or(i64::MAX).into());
    if let Some(ep) = endpoints.first() {
        fields.insert("security_mode".into(), ep.security_mode.clone().into());
        fields.insert("security_policy".into(), ep.security_policy.clone().into());
    }
    fields.insert("cap_opcua_tcp".into(), true.into());
    Some(DeviceInfo { vendor: "opcua".into(), ip: ip.into(), fields })
}

const BUILTIN_PROBES: &[VendorProbe] = &[
    VendorProbe { slug: "beckhoff", label: "Beckhoff ADS", transport: "UDP 48899 / TCP 48898", probe: probe_beckhoff },
    VendorProbe { slug: "siemens", label: "Siemens S7", transport: "TCP 102", probe: probe_siemens },
    VendorProbe { slug: "enip", label: "EtherNet/IP", transport: "TCP 44818", probe: probe_enip },
    VendorProbe { slug: "ewon", label: "eWON", transport: "UDP 1507", probe: probe_ewon },
    VendorProbe { slug: "mitsubishi", label: "Mitsubishi MELSEC", transport: "UDP 5561 / TCP 5007", probe: probe_mitsubishi },
    VendorProbe { slug: "schneider", label: "Schneider Modbus", transport: "TCP 502 / UDP", probe: probe_schneider },
    VendorProbe { slug: "phoenix", label: "Phoenix Contact", transport: "TCP 1962", probe: probe_phoenix },
    VendorProbe { slug: "omron", label: "Omron FINS", transport: "TCP/UDP 9600", probe: probe_omron },
    VendorProbe { slug: "iec104", label: "IEC 60870-5-104", transport: "TCP 2404", probe: probe_iec104 },
    VendorProbe { slug: "snmp", label: "SNMP", transport: "UDP 161", probe: probe_snmp },
    VendorProbe { slug: "mqtt", label: "MQTT", transport: "TCP 1883", probe: probe_mqtt },
    VendorProbe { slug: "bacnet", label: "BACnet/IP", transport: "UDP 47808", probe: probe_bacnet },
    VendorProbe { slug: "dnp3", label: "DNP3", transport: "TCP 20000", probe: probe_dnp3 },
    VendorProbe { slug: "opcua", label: "OPC-UA", transport: "TCP 4840", probe: probe_opcua },
];

/// Return metadata for every built-in protocol probe.
pub fn builtin_protocols() -> Vec<ProtocolInfo> {
    BUILTIN_PROBES
        .iter()
        .map(|probe| ProtocolInfo {
            slug: probe.slug,
            label: probe.label,
            transport: probe.transport,
        })
        .collect()
}

/// Run exactly one built-in protocol probe against one IP address.
///
/// This avoids an all-protocol sweep when the caller has already selected a
/// protocol, reducing unnecessary traffic in authorized OT assessments.
pub fn probe_builtin(slug: &str, ip: &str) -> Option<DeviceInfo> {
    BUILTIN_PROBES
        .iter()
        .find(|probe| probe.slug == slug)
        .and_then(|probe| (probe.probe)(ip))
}

/// An extensible registry of vendor probes used for device autodetection.
///
/// Start with [`VendorRegistry::with_builtins`] to include all 14 built-in protocols,
/// then call [`VendorRegistry::register`] to add your own vendor alongside them.
///
/// # Example
///
/// ```rust,ignore
/// use casket::core::autodetect::{VendorRegistry, VendorProbe, DeviceInfo};
///
/// let mut registry = VendorRegistry::with_builtins();
/// registry.register(VendorProbe {
///     slug: "acme",
///     label: "Acme PLC",
///     transport: "TCP 12345",
///     probe: |ip| {
///         // your detection logic here
///         None
///     },
/// });
/// let found = registry.probe_all("192.168.1.100", 8);
/// ```
pub struct VendorRegistry {
    probes: Vec<VendorProbe>,
}

impl VendorRegistry {
    /// Returns a registry pre-populated with all 14 built-in vendor probes.
    pub fn with_builtins() -> Self {
        Self { probes: BUILTIN_PROBES.to_vec() }
    }

    /// Returns an empty registry. Build your own probe set from scratch.
    pub fn empty() -> Self {
        Self { probes: Vec::new() }
    }

    /// Add a vendor probe to the registry. Returns `&mut Self` for chaining.
    pub fn register(&mut self, probe: VendorProbe) -> &mut Self {
        self.probes.push(probe);
        self
    }

    /// Probe all registered vendors in parallel against a single IP.
    ///
    /// Returns one `SweepOutcome` per registered probe. `timeout_secs` bounds the
    /// collection window; probes still running when it elapses are reported as `None`.
    pub fn sweep(&self, ip: &str, timeout_secs: u64) -> Vec<SweepOutcome> {
        use rand::Rng;
        use std::sync::mpsc;

        let stealth = stealth_enabled();
        let count = self.probes.len();

        // Build a spawn order: shuffled in stealth mode, sequential otherwise.
        let mut spawn_order: Vec<usize> = (0..count).collect();
        if stealth {
            spawn_order.shuffle(&mut rand::thread_rng());
        }

        let (tx, rx) = mpsc::channel::<(usize, Option<DeviceInfo>)>();

        for (spawn_idx, &orig_idx) in spawn_order.iter().enumerate() {
            if stealth && spawn_idx > 0 {
                thread::sleep(Duration::from_millis(rand::thread_rng().gen_range(100..=400)));
            }
            let ip = ip.to_string();
            let tx = tx.clone();
            let probe_fn = self.probes[orig_idx].probe;
            thread::spawn(move || {
                let _ = tx.send((orig_idx, probe_fn(&ip)));
            });
        }
        drop(tx);

        let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
        let mut devices: Vec<Option<DeviceInfo>> = vec![None; count];
        let mut received = 0usize;
        while received < count {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok((idx, device)) => {
                    devices[idx] = device;
                    received += 1;
                }
                Err(_) => break,
            }
        }

        self.probes
            .iter()
            .zip(devices)
            .map(|(vp, device)| SweepOutcome {
                probe: ProbeInfo {
                    label: vp.label,
                    transport: vp.transport,
                },
                device,
            })
            .collect()
    }

    /// Probe all registered vendors; return the single highest-confidence match.
    pub fn detect_device(&self, ip: &str, timeout_secs: u64) -> Option<DeviceInfo> {
        self.sweep(ip, timeout_secs)
            .into_iter()
            .filter_map(|o| o.device)
            .min_by_key(|r| vendor_priority(&r.vendor))
    }

    /// Probe all registered vendors; return every protocol that responded.
    pub fn probe_all(&self, ip: &str, timeout_secs: u64) -> Vec<DeviceInfo> {
        self.sweep(ip, timeout_secs)
            .into_iter()
            .filter_map(|o| o.device)
            .collect()
    }
}

/// Probe all built-in vendors in parallel against a single IP.
///
/// Equivalent to `VendorRegistry::with_builtins().sweep(ip, timeout_secs)`.
/// Use [`VendorRegistry`] directly to include custom vendor probes.
pub fn sweep(ip: &str, timeout_secs: u64) -> Vec<SweepOutcome> {
    VendorRegistry::with_builtins().sweep(ip, timeout_secs)
}

/// Probe all built-in vendors; return the single highest-confidence match.
///
/// Equivalent to `VendorRegistry::with_builtins().detect_device(ip, timeout_secs)`.
pub fn detect_device(ip: &str, timeout_secs: u64) -> Option<DeviceInfo> {
    VendorRegistry::with_builtins().detect_device(ip, timeout_secs)
}

/// Probe all built-in vendors; return every protocol that responded.
///
/// Equivalent to `VendorRegistry::with_builtins().probe_all(ip, timeout_secs)`.
pub fn probe_all(ip: &str, timeout_secs: u64) -> Vec<DeviceInfo> {
    VendorRegistry::with_builtins().probe_all(ip, timeout_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_protocol_catalog_matches_the_builtin_registry() {
        let protocols = builtin_protocols();
        let slugs: Vec<_> = protocols.iter().map(|protocol| protocol.slug).collect();

        assert_eq!(protocols.len(), 14);
        assert_eq!(slugs, vec![
            "beckhoff", "siemens", "enip", "ewon", "mitsubishi", "schneider", "phoenix",
            "omron", "iec104", "snmp", "mqtt", "bacnet", "dnp3", "opcua",
        ]);
        assert!(protocols.iter().all(|protocol| !protocol.label.is_empty()));
        assert!(protocols.iter().all(|protocol| !protocol.transport.is_empty()));
    }

    #[test]
    fn unsupported_builtin_probe_returns_none_without_network_io() {
        assert!(probe_builtin("not-a-protocol", "127.0.0.1").is_none());
    }
}
