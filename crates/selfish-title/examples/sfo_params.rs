//! Every parameter in a `param.sfo`, with the kind the file states and the bytes it holds, then
//! each named key (and `ACCOUNT_ID`) as raw hex.
//!
//! ```text
//! cargo run -p selfish-title --example sfo_params -- <param.sfo> [KEY]...
//! ```

// A probe reads fixed offsets and prints them, so the library's arithmetic lints do not apply.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::uninlined_format_args,
    clippy::format_collect,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout
)]

use selfish_title::sfo::{Format, Sfo, Value};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: sfo_params <param.sfo> [KEY]...")?;
    let mut wanted: Vec<String> = args.collect();
    if wanted.is_empty() {
        wanted.push("ACCOUNT_ID".to_owned());
    }

    let sfo = Sfo::parse(&std::fs::read(&path)?)?;
    println!("{}  {} parameters\n", path, sfo.entries().len());

    for entry in sfo.entries() {
        let kind = match entry.format() {
            Format::Utf8 => "text".to_owned(),
            Format::Utf8Special => "unterminated".to_owned(),
            Format::Integer => "integer".to_owned(),
            Format::Other(code) => format!("format {code:#06x}"),
        };
        // The variant, shown apart from the format code: an unterminated value reads as text
        // or as bytes depending on whether it decodes.
        let read_as = match &entry.value {
            Value::Text(text) | Value::TextUnterminated(text) => format!("{text:?}"),
            Value::Integer(number) => format!("{number} ({number:#x})"),
            Value::Binary(bytes) => format!("{} bytes, not text", bytes.len()),
            Value::Unknown(_, bytes) => format!("{} bytes, uninterpreted", bytes.len()),
        };
        println!(
            "  {:<20} {:<14} value {:<5} reserved {:<5} {}",
            entry.key,
            kind,
            // The value's own bytes; for a terminated string, one less than the index's
            // `length`.
            entry.value.as_bytes().map_or(4, <[u8]>::len),
            entry.reserved,
            read_as
        );
    }

    println!();
    for key in &wanted {
        match sfo.bytes(key) {
            // Hex in file order; the byte order of a user id is the consumer's choice.
            Some(bytes) => println!(
                "  {key} = {} ({} bytes, as written)",
                hex(bytes),
                bytes.len()
            ),
            None if sfo.get(key).is_some() => {
                println!("  {key} is an integer - read it with `as_integer`");
            }
            None => println!("  {key} is not in this file"),
        }
    }
    Ok(())
}
