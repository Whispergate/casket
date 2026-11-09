# Architecture

`casket` is one library crate (`casket`) plus one proc-macro crate (`casket-macros`). The split is by
responsibility, not by vendor: a protocol module owns its own bytes, and the shared machinery knows
nothing about PLC models. Everything below was read out of this tree on 2026-11-09, not from memory.

## The four layers

**`core`** (`src/core/`) is transport and shared machinery. `network.rs` lists interfaces, `portscan.rs`
does a TCP sweep, `bytes.rs` holds pack/unpack helpers, `capture.rs` reads pcaps behind the `capture`
feature, `modbus.rs` is the Modbus codec, and `modbus_fdi.rs` / `modbus_server.rs` are the attack and
simulation helpers built on it. `shellshock.rs` and `httpcreds.rs` are HTTP-level checks. None of these
know a PLC model. The one exception, `autodetect.rs`, is the composition root described below.

**`vendors`** (`src/vendors/<protocol>/`) is the per-protocol codec layer: request builders, response
parsers, scan probes, and the exploit paths that only make sense for one protocol. Every directory has
`mod.rs` and generally one of `client.rs`, `scan.rs` or `session.rs`; `scripts/check-module-shape.sh`
enforces that, and currently reports `phoenix` (`control.rs`, `webvisit.rs`) and `rockwell`
(`driver.rs`) as debt because they name their entry points differently. A vendor module depends on
`crate::core` helpers and `crate::error`, and never on `crate::prelude`.

**`prelude`** (`src/prelude.rs`) is the public door: 19 lines re-exporting `Error`/`Result`, the
autodetect entry points (`detect_device`, `probe_all`, `sweep`, `DeviceInfo`, `VendorRegistry`, ...),
and the handful of vendor types a first caller needs (`BeckhoffDevice`, `MqttSession`, `OmronDevice`,
`SiemensDevice`, `SlmpValue`). Add to it only what belongs in a hello-world; everything else stays
under `casket::vendors`, so the prelude does not become a second, unversioned API surface.

**`macros`** (`casket-macros/`) is the proc-macro crate, behind the optional `macros` feature. It has
one derive, `IntoDeviceInfo`, which fills the `HashMap<String, serde_json::Value>` that
`casket::core::autodetect::DeviceInfo` carries, using the struct-level `#[vendor(slug = "...")]` and
field-level `#[device_info(ip | skip | rename | optional)]` attributes. It is a separate crate only
because a proc-macro has to be.

## How a probe reaches a vendor module

`detect_device(ip, timeout)` builds `VendorRegistry::with_builtins()`, whose `sweep()` walks the
`BUILTIN_PROBES` table at `src/core/autodetect.rs:529`, spawns one thread per probe over an `mpsc`
channel, and shuffles the order plus adds 100 to 400 ms of jitter when `set_stealth(true)` is in
effect. Each entry carries `slug`, `label`, `transport`, and `probe: fn(&str) -> Option<DeviceInfo>`.
That function is the only adapter between the registry and a protocol module: `probe_siemens`
(`src/core/autodetect.rs:153`) dials the port itself and then calls `s7comm` helpers, `probe_enip`
(`:184`) calls `vendors::enip::scan::scan_ip` (`src/vendors/enip/scan.rs:125`) and takes the first
`EnipDevice`, then flattens it into `DeviceInfo::fields` as `port`, `*_port` and `cap_*` keys. Anything
that errors or times out becomes `None`; the probe never panics into the sweep. When the channel
drains, `detect_device` picks the result with the lowest `vendor_priority` (`modicon` 0, the
make-specific slugs 1, `enip`/`iec104` 2, `snmp`/`mqtt` 3) and `probe_all` returns every hit.
`probe_builtin(slug, ip)` runs the same function through the same table without starting a sweep.

Adding a protocol therefore means: a `src/vendors/<name>/` module, one `probe_*` function in
`autodetect.rs`, one row in `BUILTIN_PROBES`, and at most one line in the prelude. There is no build
script and no registration macro to update.

## The 16 protocols

"Start here" is the file a contributor should open first: the codec for request/response protocols, the
probe for scan-first ones. Every port is a `const` in the tree, cited as `file:line`; none was guessed.

| # | Protocol | Start here | Default port | Port const (file:line) | Other ports in the module |
|---|---|---|---|---|---|
| 1 | EtherNet/IP (ENIP/CIP) | `src/vendors/enip/scan.rs` | 44818 | `DEST_PORT`, `src/vendors/enip/scan.rs:10` | |
| 2 | Modbus TCP | `src/core/modbus.rs` | 502 | `DEFAULT_PORT`, `src/core/modbus.rs:7` | |
| 3 | S7Comm | `src/vendors/siemens/s7comm.rs` | 102 | `S7_PORT`, `src/vendors/siemens/s7comm.rs:9` | |
| 4 | FINS TCP/UDP | `src/vendors/omron/fins.rs` | 9600 | `FINS_TCP_PORT`, `src/vendors/omron/fins.rs:11` | UDP uses the same 9600, `FINS_UDP_PORT`, `src/vendors/omron/fins.rs:13` |
| 5 | SLMP | `src/vendors/mitsubishi/slmp.rs` | 5007 | `DEFAULT_PORT`, `src/vendors/mitsubishi/slmp.rs:7` | UDP discovery 5561 (`scan.rs:8`), SLMP discovery 5006 (`scan.rs:9`), control 5560 (`control.rs:17`) |
| 6 | ADS/AMS | `src/vendors/beckhoff/ads.rs` | 48898 | `DEFAULT_ADS_PORT`, `src/vendors/beckhoff/scan.rs:14` | UDP discovery 48899 (`scan.rs:13`), web 5120 (`webcontrol.rs:25`) |
| 7 | IEC 60870-5-104 | `src/vendors/iec104/client.rs` | 2404 | `IEC104_PORT`, `src/vendors/iec104/client.rs:11` | |
| 8 | SNMP | `src/vendors/snmp/client.rs` | 161 | `SNMP_PORT`, `src/vendors/snmp/client.rs:7` | |
| 9 | MQTT | `src/vendors/mqtt/client.rs` | 1883 | `MQTT_PORT`, `src/vendors/mqtt/client.rs:12` | |
| 10 | DNP3 | `src/vendors/dnp3/client.rs` | 20000 | `DNP3_PORT`, `src/vendors/dnp3/client.rs:13` | |
| 11 | BACnet/IP | `src/vendors/bacnet/client.rs` | 47808 | `BACNET_PORT` (`0xBAC0`), `src/vendors/bacnet/client.rs:11` | |
| 12 | OPC-UA | `src/vendors/opcua/client.rs` | 4840 | `OPCUA_PORT`, `src/vendors/opcua/client.rs:13` | |
| 13 | eWON | `src/vendors/ewon/scan.rs` | 1507 | `IPCONF_DISCOVERY_PORT`, `src/vendors/ewon/scan.rs:8` | response 1506 (`scan.rs:9`), HTTP 80 as a literal (`exploit.rs:67`) |
| 14 | Phoenix Contact (control info, WebVisit) | `src/vendors/phoenix/control.rs` | 1962 | `INFO_PORT`, `src/vendors/phoenix/control.rs:6` | ILC150 41100 (`control.rs:7`), ILC390 20547 (`control.rs:8`), WebVisit HTTP 80 as a literal (`webvisit.rs:14`) |
| 15 | Modicon FC90 (Schneider) | `src/vendors/schneider/modicon_fc90.rs` | 502 | `FC90_PORT`, `src/vendors/schneider/modicon_fc90.rs:11` | UDP discovery 1740 (`src/vendors/schneider/scan.rs:8`), LED flash 27127 (`flash_led.rs:15`) |
| 16 | CIP over EtherNet/IP (Rockwell tag driver) | `src/vendors/rockwell/driver.rs` | 44818 | `EIP_PORT`, `src/vendors/rockwell/driver.rs:7` | |

These are the 16 rows the README advertises. `BUILTIN_PROBES` has 14 entries, not 16: Rockwell is
reached through the ENIP probe, which relabels vendor ids 1, 5 and 77 as `"rockwell"`
(`src/core/autodetect.rs:191`), and Modicon FC90 is reached through the Schneider probe.

To re-check one row:

```sh
sed -n '10p' src/vendors/enip/scan.rs   # const DEST_PORT: u16 = 44818;
```

## Where the layers bend

- `core` is not strictly underneath `vendors`. `src/core/autodetect.rs` imports 14 vendor modules and
  `src/core/httpcreds.rs` imports the vendor credential tables, so `autodetect` is a composition root
  that sits on top of both layers. Every other `core` file is below `vendors`.
- Some probes repeat their port as a literal instead of using the module const:
  `format!("{ip}:102")` at `src/core/autodetect.rs:159`, `48898` at `:134`, `5007` at `:254`, `1962` at
  `:361`. The const is the source of truth inside the module, but changing one does not change what
  the probe dials. Grep `autodetect.rs` when you touch a port.
- Modbus is the only protocol outside the vendor tree (`src/core/modbus.rs`). It is there because the
  Modbus codec is shared by the Schneider module, the server helper and the FDI helper.
- Port `0` means "use the module default" at the vendor entry points (`if port == 0 { CONST }`), for
  example `src/vendors/phoenix/control.rs:82` and `src/vendors/omron/fins.rs:65`. Callers that pass 0
  get the const above; callers with a real port override it.

## Reading order for a new contributor

1. `src/lib.rs` (crate doc, five `pub mod` lines) and `src/prelude.rs`, to see the advertised surface.
2. `src/error.rs`, then `docs/errors.md`, because every module returns `crate::error::Result`.
3. `src/core/autodetect.rs` from `DeviceInfo` (line 31) to `BUILTIN_PROBES` (line 529), which is the
   whole detection story in about 500 lines.
4. One vendor module end to end. `src/vendors/dnp3/client.rs` (552 lines) is a complete TCP protocol
   with its probe in one file; `src/vendors/enip/scan.rs` shows the scan-first shape instead.
