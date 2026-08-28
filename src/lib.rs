//! `casket` — ICS/OT protocol implementations and vendor plugin APIs.
//!
//! Provides protocol implementations for EtherNet/IP, Modbus, `S7Comm`, FINS, SLMP,
//! ADS/AMS, IEC 60870-5-104, SNMP, MQTT, DNP3, `BACnet`, and OPC-UA across major PLC
//! vendors. Includes device autodetection, stealth scanning, and an extensible
//! [`VendorRegistry`](core::autodetect::VendorRegistry) for third-party vendor plugins.
//!
//! **For authorized lab and CTF use only.**

pub mod core;
pub mod prelude;
pub mod references;
pub mod vendors;
