//! Read a device that this process started itself, so the example needs no PLC and no network.
//!
//!     cargo run --example loopback_modbus
//!
//! `core::modbus_server` answers FC3 with a fixed value, which makes it useful as a test double.

use casket::core::modbus::read_holding_registers;
use casket::core::modbus_server::serve;
use std::time::Duration;

const PORT: u16 = 15_020;

fn main() {
    std::thread::spawn(move || {
        // The server runs until the process exits; the Result is reported, not unwrapped, because
        // a busy port is a normal outcome when this example is run twice.
        if let Err(error) = serve(PORT, 0x1234, true) {
            eprintln!("server stopped: {error}");
        }
    });
    std::thread::sleep(Duration::from_millis(300));

    match read_holding_registers("127.0.0.1", PORT, 0, 4) {
        Ok(registers) => {
            println!(
                "read {} register(s) from the in-process server",
                registers.len()
            );
            for register in registers {
                println!("  {} = {}", register.display_addr, register.value_str);
            }
        }
        Err(error) => {
            eprintln!("read failed: {error}");
            std::process::exit(1);
        }
    }
}
