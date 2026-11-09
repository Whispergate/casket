//! Read holding registers (FC 0x03) from a Modbus TCP device.
//!
//!     cargo run --example read_modbus -- 192.168.1.10
//!     cargo run --example read_modbus -- 192.168.1.10 502 0 8
//!
//! Address 0 here is protocol address 0, displayed as 40001. Count is capped at 125 by the protocol.

use casket::core::modbus::read_holding_registers;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(host) = args.next() else {
        eprintln!("usage: read_modbus <ip> [port] [start] [count]");
        std::process::exit(2);
    };
    let port: u16 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(502);
    let start: u16 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let count: u16 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(4);

    match read_holding_registers(&host, port, start, count) {
        Ok(registers) => {
            println!("{host}:{port} returned {} register(s)", registers.len());
            for register in registers {
                println!(
                    "  {} (proto {}) = {}",
                    register.display_addr, register.address, register.value_str
                );
            }
        }
        Err(error) => {
            eprintln!("read failed: {error}");
            std::process::exit(1);
        }
    }
}
