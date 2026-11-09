//! Autodetect one host and print whatever answered.
//!
//!     cargo run --example detect -- 192.168.1.10
//!     cargo run --example detect -- 192.0.2.1 1    # silent address, 1 s budget

use casket::core::autodetect::detect_device;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(target) = args.next() else {
        eprintln!("usage: detect <ip> [timeout_secs]");
        std::process::exit(2);
    };
    let timeout_secs: u64 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);

    match detect_device(&target, timeout_secs) {
        Some(device) => {
            println!("{} answered as {}", device.ip, device.vendor);
            let mut fields: Vec<_> = device.fields.iter().collect();
            fields.sort_by(|left, right| left.0.cmp(right.0));
            for (key, value) in fields {
                println!("  {key}: {value}");
            }
        }
        None => println!("{target}: nothing answered within {timeout_secs}s"),
    }
}
