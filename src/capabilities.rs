//! What each protocol can do, as data rather than prose.
//!
//! The Python side of the toolchain answers "can this protocol read, can it write" from its own
//! table. This module is the Rust answer, so the two cannot drift silently. The row set is checked
//! against the vendor directories at test time: adding `src/vendors/<name>/` without a row here
//! fails `cargo test`.
//!
//! `write` means a `write_*` function exists for the protocol, per `grep -rn 'pub fn write' src`.
//! Protocols that can change device state through a control, publish or `set` path but have no
//! `write_*` function report `write: false` and name that path in `notes`; the flag is a statement
//! about the API shape, not about what the protocol is capable of.

/// One protocol's read/write surface.
pub struct Capability {
    /// Vendor directory name under `src/vendors/`, or the module name for a protocol outside it.
    pub protocol: &'static str,
    /// A read path exists: tag, register, attribute, identity or scan.
    pub read: bool,
    /// A `write_*` function exists. See the module docs for what this does and does not mean.
    pub write: bool,
    /// Entry file, and for `write: true` rows the function that proves it.
    pub notes: &'static str,
}

// Rows follow the README's protocol order. A `write: true` row names the function that proves it;
// a protocol with a control, publish or `set` path but no `write_*` function stays false and says
// so in `notes`.
const CAPABILITIES: [Capability; 16] = [
    // Read evidence: `scan_ip` / `ListIdentity` in `src/vendors/enip/scan.rs`. No `write_*`.
    Capability {
        protocol: "enip",
        read: true,
        write: false,
        notes: "EtherNet/IP (ENIP/CIP) identity and stack scan; entry src/vendors/enip/scan.rs.",
    },
    // Write evidence: `write_single_coil`, `write_single_register`, `write_multiple_registers`
    // in `src/core/modbus.rs`.
    Capability {
        protocol: "modbus",
        read: true,
        write: true,
        notes: "Modbus TCP codec in core, not the vendor tree; entry src/core/modbus.rs.",
    },
    // Write evidence: `write_data_block`, `write_output_bit`, `write_merker_bit` in
    // `src/vendors/siemens/s7comm.rs`.
    Capability {
        protocol: "siemens",
        read: true,
        write: true,
        notes: "S7Comm; entry src/vendors/siemens/s7comm.rs (read_data_block, write_data_block).",
    },
    // Write evidence: `write_dm_words` in `src/vendors/omron/fins.rs`.
    Capability {
        protocol: "omron",
        read: true,
        write: true,
        notes: "FINS TCP/UDP; entry src/vendors/omron/fins.rs (read_dm_words, write_dm_words).",
    },
    // Write evidence: `write_word_devices`, `write_bit_devices` in `src/vendors/mitsubishi/slmp.rs`.
    Capability {
        protocol: "mitsubishi",
        read: true,
        write: true,
        notes: "SLMP; entry src/vendors/mitsubishi/slmp.rs (read_word_devices, write_word_devices).",
    },
    // Write evidence: `write_symbol_value` in `src/vendors/beckhoff/scan.rs`.
    Capability {
        protocol: "beckhoff",
        read: true,
        write: true,
        notes: "ADS/AMS; entry src/vendors/beckhoff/ads.rs; write_symbol_value in src/vendors/beckhoff/scan.rs.",
    },
    // Read evidence: `general_interrogation` in `src/vendors/iec104/client.rs`. No `write_*`.
    Capability {
        protocol: "iec104",
        read: true,
        write: false,
        notes: "IEC 60870-5-104; entry src/vendors/iec104/client.rs. Control is single_command, not write_*.",
    },
    // Read evidence: `get`, `walk` in `src/vendors/snmp/client.rs`. `set` exists but is not `write_*`.
    Capability {
        protocol: "snmp",
        read: true,
        write: false,
        notes: "SNMP; entry src/vendors/snmp/client.rs. set() changes state but is not write_*.",
    },
    // Read evidence: `subscribe`, `drain_messages` in `src/vendors/mqtt/client.rs`.
    Capability {
        protocol: "mqtt",
        read: true,
        write: false,
        notes: "MQTT; entry src/vendors/mqtt/client.rs. publish() writes a topic but is not write_*.",
    },
    // Read evidence: `read_integrity_poll`, `read_device_attributes` in `src/vendors/dnp3/client.rs`.
    Capability {
        protocol: "dnp3",
        read: true,
        write: false,
        notes: "DNP3; entry src/vendors/dnp3/client.rs (read_integrity_poll, read_device_attributes).",
    },
    // Read evidence: `read_property` in `src/vendors/bacnet/client.rs`.
    Capability {
        protocol: "bacnet",
        read: true,
        write: false,
        notes: "BACnet/IP; entry src/vendors/bacnet/client.rs (read_property, device_summary).",
    },
    // Read evidence: `get_endpoints` in `src/vendors/opcua/client.rs`.
    Capability {
        protocol: "opcua",
        read: true,
        write: false,
        notes: "OPC-UA; entry src/vendors/opcua/client.rs (get_endpoints, server_summary).",
    },
    // Read evidence: `read_tag_values` in `src/vendors/ewon/scan.rs`.
    Capability {
        protocol: "ewon",
        read: true,
        write: false,
        notes: "eWON Flexy; entry src/vendors/ewon/scan.rs (read_tag_values).",
    },
    // Write evidence: `write_tag_value` in `src/vendors/phoenix/webvisit.rs`.
    Capability {
        protocol: "phoenix",
        read: true,
        write: true,
        notes: "Phoenix Contact WebVisit; entry src/vendors/phoenix/webvisit.rs (read_tag_values, write_tag_value).",
    },
    // `force_output_bit` and `control_plc` change device state but are not `write_*`.
    Capability {
        protocol: "schneider",
        read: true,
        write: false,
        notes: "Modicon FC90; entry src/vendors/schneider/modicon_fc90.rs. force_output_bit is control, not write_*.",
    },
    // Write evidence: `write_tag` in `src/vendors/rockwell/driver.rs`.
    Capability {
        protocol: "rockwell",
        read: true,
        write: true,
        notes: "CIP tag driver over EtherNet/IP; entry src/vendors/rockwell/driver.rs (read_tag, write_tag).",
    },
];

/// Every protocol in the crate, in the order the README advertises them.
pub const fn capabilities() -> &'static [Capability] {
    &CAPABILITIES
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    /// Directory names under `src/vendors/`, read at test time so a new vendor is not invisible.
    fn vendor_directories() -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("vendors");
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        let mut names: Vec<String> = entries
            .filter_map(|entry| {
                let entry = entry.expect("directory entry");
                entry.file_type().expect("file type").is_dir().then(|| {
                    entry.file_name().into_string().expect("vendor directory name is UTF-8")
                })
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn every_vendor_directory_has_a_capability() {
        let rows = capabilities();
        let missing: Vec<String> = vendor_directories()
            .into_iter()
            .filter(|name| !rows.iter().any(|row| row.protocol == name))
            .collect();
        assert!(missing.is_empty(), "protocol directories with no capability row: {missing:?}");
    }

    #[test]
    fn protocol_names_are_unique() {
        let mut seen = BTreeSet::new();
        for row in capabilities() {
            assert!(seen.insert(row.protocol), "duplicate capability row for {}", row.protocol);
        }
    }

    #[test]
    fn every_row_names_its_entry_file() {
        for row in capabilities() {
            assert!(!row.notes.is_empty(), "{} has no notes", row.protocol);
            assert!(row.notes.contains("src/"), "{} does not name an entry file: {}", row.protocol, row.notes);
        }
    }
}
