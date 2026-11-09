//! Print the protocol support matrix that this build exposes.
//!
//!     cargo run --example capabilities
//!
//! The output is Markdown so it can be pasted into documentation, and it comes from the same table
//! the library uses to answer `write` questions, so the two cannot disagree.
//!
//! `write` means a `write_*` function exists, not that a device can be changed: SNMP (`set`), MQTT
//! (`publish`), IEC-104 (`single_command`) and Schneider (`force_output_bit`) change state without one.

fn main() {
    println!("| Protocol | Read | Write | Notes |");
    println!("|---|---|---|---|");
    for row in casket::capabilities::capabilities() {
        println!(
            "| `{}` | {} | {} | {} |",
            row.protocol,
            yes_no(row.read),
            yes_no(row.write),
            row.notes
        );
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}
