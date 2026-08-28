# casket

A Rust library of ICS/OT protocol implementations and vendor plugin APIs.
It is the protocol and vendor layer extracted from [SCADAVER](https://github.com/Whispergate/SCADAVER)
so the implementations can be used independently of the tool.

## What it provides

`casket` contains pure-Rust implementations of 16 industrial control system protocols,
device autodetection across major PLC and HMI vendors, and an extensible `VendorRegistry`
so third parties can register their own vendor probes alongside the built-ins.

**Supported protocols and vendors**

| Protocol | Vendor |
|---|---|
| EtherNet/IP (ENIP/CIP) | Rockwell Automation / Allen-Bradley |
| Modbus TCP | Generic |
| S7Comm | Siemens |
| FINS TCP/UDP | Omron |
| SLMP | Mitsubishi |
| ADS/AMS | Beckhoff |
| IEC 60870-5-104 | Generic RTU/MTU |
| SNMP | Generic |
| MQTT | Broker enumeration |
| DNP3 | Generic |
| BACnet | Generic |
| OPC-UA | Generic |
| eWON | HMS Networks |
| WebVisit | Phoenix Contact |
| Modicon FC90 | Schneider Electric |
| CIP over EtherNet/IP | Rockwell (tag driver) |

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
casket = { git = "https://github.com/Whispergate/casket" }
```

### Autodetect a device

```rust
use casket::core::autodetect::detect_device;

fn main() {
    if let Some(info) = detect_device("192.168.1.10", 5) {
        println!("Vendor: {}", info.vendor);
        for (k, v) in &info.fields {
            println!("  {k}: {v}");
        }
    }
}
```

### Register a custom vendor probe

```rust
use casket::core::autodetect::{DeviceInfo, VendorProbe, VendorRegistry};

fn probe_acme(ip: &str) -> Option<DeviceInfo> {
    // your detection logic here
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

    let results = registry.probe_all("192.168.1.10", 5);
    for device in &results {
        println!("{} at {}", device.vendor, device.ip);
    }
}
```

Run the bundled example:

```bash
cargo run --example custom_vendor -- 192.168.1.10
```

### Optional features

| Feature | Enables |
|---|---|
| `macros` | `#[derive(IntoDeviceInfo)]` proc-macro for vendor structs |
| `tls` | TLS transport for MQTT sessions |
| `capture` | Live packet capture via `libpcap` |

## Architecture

```
casket/
  src/
    core/
      autodetect.rs    VendorRegistry, VendorProbe, DeviceInfo
      network.rs       Interface enumeration, broadcast UDP sockets
      capture.rs       Packet capture helpers (feature = "capture")
      modbus.rs        Modbus codec shared by multiple vendors
    vendors/
      bacnet/          BACnet/IP
      beckhoff/        ADS/AMS
      dnp3/            DNP3 serial and TCP
      enip/            EtherNet/IP, CIP
      ewon/            HMS eWON Cosy/Flexy
      iec104/          IEC 60870-5-104
      mitsubishi/      SLMP
      mqtt/            MQTT broker scan and session
      omron/           FINS TCP/UDP
      opcua/           OPC-UA
      phoenix/         Phoenix Contact WebVisit
      rockwell/        Allen-Bradley tag read/write
      schneider/       Modicon FC90, session hijack
      siemens/         S7Comm
      snmp/            SNMP v1/v2c/v3
    prelude.rs         Re-exports for the most common types
    references.rs      CVE and protocol reference data
  casket-macros/       Proc-macro workspace member
```

## Writing a vendor plugin

A vendor probe is a plain function with the signature `fn(&str) -> Option<DeviceInfo>`.
No traits to implement and no registration macros required.
Register it at runtime with `VendorRegistry::register()`.

For vendors where you build `DeviceInfo` from a struct, the `macros` feature provides
`#[derive(IntoDeviceInfo)]`:

```toml
[dependencies]
casket = { git = "https://github.com/Whispergate/casket", features = ["macros"] }
```

```rust
use casket_macros::IntoDeviceInfo;
use casket::core::autodetect::IntoDeviceInfo;

#[derive(IntoDeviceInfo)]
#[vendor(slug = "acme", label = "Acme PLC")]
struct AcmeDevice {
    firmware: String,
    model: String,
}
```

## License

MIT. See [LICENSE](LICENSE).

For authorized lab and research use only.
