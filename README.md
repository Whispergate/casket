# casket

A Rust library of ICS/OT protocol implementations and vendor plugin APIs.
It is the protocol and vendor layer extracted from [SCADAVER](https://github.com/Whispergate/SCADAVER)
so the implementations can be used independently of the tool.

## What it provides

`casket` contains pure-Rust implementations of 15 industrial control system protocol families
(16 support rows, because EtherNet/IP appears twice: identity scanning and the CIP tag driver),
device autodetection across major PLC and HMI vendors, and an extensible `VendorRegistry`
so third parties can register their own vendor probes alongside the built-ins.

**Support matrix.** `cargo run --example capabilities` prints this table from the same data the
library uses to answer questions about itself, so it cannot drift from the code.

| Protocol | Read | Write | Notes |
|---|---|---|---|
| `enip` | yes | no | EtherNet/IP (ENIP/CIP) identity and stack scan; entry src/vendors/enip/scan.rs. |
| `modbus` | yes | yes | Modbus TCP codec in core, not the vendor tree; entry src/core/modbus.rs. |
| `siemens` | yes | yes | S7Comm; entry src/vendors/siemens/s7comm.rs (read_data_block, write_data_block). |
| `omron` | yes | yes | FINS TCP/UDP; entry src/vendors/omron/fins.rs (read_dm_words, write_dm_words). |
| `mitsubishi` | yes | yes | SLMP; entry src/vendors/mitsubishi/slmp.rs (read_word_devices, write_word_devices). |
| `beckhoff` | yes | yes | ADS/AMS; entry src/vendors/beckhoff/ads.rs; write_symbol_value in src/vendors/beckhoff/scan.rs. |
| `iec104` | yes | no | IEC 60870-5-104; entry src/vendors/iec104/client.rs. Control is single_command, not write_*. |
| `snmp` | yes | no | SNMP; entry src/vendors/snmp/client.rs. set() changes state but is not write_*. |
| `mqtt` | yes | no | MQTT; entry src/vendors/mqtt/client.rs. publish() writes a topic but is not write_*. |
| `dnp3` | yes | no | DNP3; entry src/vendors/dnp3/client.rs (read_integrity_poll, read_device_attributes). |
| `bacnet` | yes | no | BACnet/IP; entry src/vendors/bacnet/client.rs (read_property, device_summary). |
| `opcua` | yes | no | OPC-UA; entry src/vendors/opcua/client.rs (get_endpoints, server_summary). |
| `ewon` | yes | no | eWON Flexy; entry src/vendors/ewon/scan.rs (read_tag_values). |
| `phoenix` | yes | yes | Phoenix Contact WebVisit; entry src/vendors/phoenix/webvisit.rs (read_tag_values, write_tag_value). |
| `schneider` | yes | no | Modicon FC90; entry src/vendors/schneider/modicon_fc90.rs. force_output_bit is control, not write_*. |
| `rockwell` | yes | yes | CIP tag driver over EtherNet/IP; entry src/vendors/rockwell/driver.rs (read_tag, write_tag). |

`Write` means a `write_*` function exists, not that a device accepts the write. Four protocols change
state through a verb that is not named `write_*`: SNMP `set`, MQTT `publish`, IEC-104
`single_command`, Schneider `force_output_bit`.

## Install

```bash
cargo add casket
```

Rust 1.88 or newer. To follow unreleased work, pin the repository instead:

```toml
[dependencies]
casket = { git = "https://github.com/Whispergate/casket" }
```

API reference: <https://docs.rs/casket>

## Usage

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

## Examples

Each example is a standalone binary with no extra dependencies, and each one runs with
`cargo run --example <name>`:

| Example | What it shows |
|---|---|
| `detect` | Autodetect one host and print the vendor and the fields that answered |
| `read_modbus` | Read holding registers from a device on the network |
| `write_modbus` | Write one register, read it back, and refuse to claim success unless the value stuck |
| `loopback_modbus` | Read a device the process starts itself, so the whole path runs without hardware |
| `capabilities` | Print the support matrix above |
| `custom_vendor` | Register a vendor probe beside the built-ins |

```bash
cargo run --example loopback_modbus
cargo run --example detect -- 192.168.1.10 5
cargo run --example read_modbus -- 192.168.1.10 502 0 8
cargo run --example write_modbus -- 192.168.1.10 502 10 1234 --i-am-sure
```

The `--i-am-sure` flag is deliberate friction. A register write moves a real setpoint, and the example
refuses to run without it.

### Optional features

| Feature | Enables |
|---|---|
| `macros` | `#[derive(IntoDeviceInfo)]` proc-macro for vendor structs |
| `tls` | TLS transport for MQTT sessions |
| `capture` | Live packet capture via `libpcap` |

## Architecture

```
casket/
  src/core/           autodetect, network, capture, modbus codec, device client trait, error type
  src/vendors/        one directory per protocol family
  src/capabilities.rs the support matrix, as data
  casket-macros/      #[derive(IntoDeviceInfo)]
```

[docs/architecture.md](docs/architecture.md) has the layer map and why the boundaries fall where they
do; [docs/errors.md](docs/errors.md) covers the error type.

## Writing a vendor plugin

A vendor probe is a plain function with the signature `fn(&str) -> Option<DeviceInfo>`.
No traits to implement and no registration macros required.
Register it at runtime with `VendorRegistry::register()`.

For vendors where you build `DeviceInfo` from a struct, the `macros` feature provides
`#[derive(IntoDeviceInfo)]`:

```toml
[dependencies]
casket = { version = "0.1.2", features = ["macros"] }
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
