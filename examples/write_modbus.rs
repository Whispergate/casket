//! Write one holding register, then read it back and compare.
//!
//!     cargo run --example write_modbus -- 192.168.1.10 502 10 1234 --i-am-sure
//!
//! A write without a read-back is a write you cannot report, so this example refuses to claim success
//! without the second read. The `--i-am-sure` flag is deliberate friction: pointing this at the wrong
//! address moves a real setpoint.

use casket::core::modbus::{read_holding_registers, write_single_register};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|arg| arg == "--i-am-sure") {
        eprintln!("refusing to write without --i-am-sure");
        eprintln!("usage: write_modbus <ip> [port] <address> <value> --i-am-sure");
        std::process::exit(2);
    }
    let positional: Vec<&String> = args.iter().filter(|arg| !arg.starts_with("--")).collect();
    if positional.len() < 2 {
        eprintln!("usage: write_modbus <ip> [port] <address> <value> --i-am-sure");
        std::process::exit(2);
    }

    let host = positional[0];
    let (port, address, value) = match positional.len() {
        3 => (502u16, parse(positional[1]), parse(positional[2])),
        _ => (
            parse(positional[1]),
            parse(positional[2]),
            parse(positional[3]),
        ),
    };

    if let Err(error) = write_single_register(host, port, address, value) {
        eprintln!("write failed: {error}");
        std::process::exit(1);
    }

    match read_holding_registers(host, port, address, 1) {
        Ok(registers) if registers.first().map(|register| register.raw) == Some(value) => {
            println!("{host}:{port} register {address} now reads {value}, confirmed");
        }
        Ok(registers) => {
            let actual = registers.first().map_or_else(
                || "nothing".to_string(),
                |register| register.value_str.clone(),
            );
            eprintln!("wrote {value} but read back {actual}");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("wrote {value} but the read-back failed: {error}");
            std::process::exit(1);
        }
    }
}

fn parse(text: &str) -> u16 {
    let Ok(value) = text.parse() else {
        eprintln!("{text} is not a 16-bit number");
        std::process::exit(2);
    };
    value
}
