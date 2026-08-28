//! Demonstrates how to register a third-party vendor probe alongside the built-ins.
//!
//! Run with:
//!   cargo run --example custom_vendor

use casket::core::autodetect::{DeviceInfo, VendorProbe, VendorRegistry};

fn probe_acme(ip: &str) -> Option<DeviceInfo> {
    // Replace with real detection logic (TCP connect, banner grab, etc.)
    // Return None if the target does not look like this vendor.
    let _ = ip;
    None
}

fn main() {
    let mut registry = VendorRegistry::with_builtins();
    registry.register(VendorProbe {
        slug: "acme",
        label: "Acme PLC",
        transport: "TCP 12345",
        probe: probe_acme,
    });

    let target = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1".to_string());
    println!("Probing {target} with {} vendors …", registry.probe_all(&target, 0).len());

    let found = registry.probe_all(&target, 8);
    if found.is_empty() {
        println!("No devices found.");
    } else {
        for device in &found {
            println!("  {} at {}", device.vendor, device.ip);
        }
    }
}
