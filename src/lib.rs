//! `casket` — ICS/OT protocol implementations and vendor plugin APIs.
//!
//! Provides protocol implementations for EtherNet/IP, Modbus, `S7Comm`, FINS, SLMP,
//! ADS/AMS, IEC 60870-5-104, SNMP, MQTT, DNP3, `BACnet`, and OPC-UA across major PLC
//! vendors. Includes device autodetection, stealth scanning, and an extensible
//! [`VendorRegistry`](core::autodetect::VendorRegistry) for third-party vendor plugins.
//!
//! **For authorized lab and CTF use only.**

// The documentation floor is measured on doc builds, not on every build: `clippy -- -D warnings`
// escalates each missing-doc warning into an error, and 343 items are undocumented today.
// See the `# missing_docs` line at the end of docs/api-surface.txt.
#![cfg_attr(doc, warn(missing_docs))]

pub mod capabilities;
pub mod core;
pub mod error;
pub mod prelude;
pub mod references;
pub mod vendors;

pub use error::{Error, Result};
