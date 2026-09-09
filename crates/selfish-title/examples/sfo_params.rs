//! Every parameter in a `param.sfo`, with the kind the file states and the bytes it holds.
//!
//! The reason to run this rather than read the struct is that a parameter's *kind* is where
//! this format surprises people. `utf8_special` is a length and no terminator, and it carries
//! PS3 text and the current generation's `ACCOUNT_ID` alike - eight bytes of user id that are
//! usually not text at all. So "what kind is this key" is a question about the file in front
//! of you, not about the format, and this prints the answer.
//!
//! ```text
//! cargo run -p selfish-title --example sfo_params -- <param.sfo> [KEY]...
//! ```
//!
//! Named keys are printed as raw hex afterwards, which is the shape a consumer wants for an
//! id it is going to compare rather than display. `ACCOUNT_ID` is included by default because
//! it is the one every caller asks for first.

// A diagnostic probe, held to a probe's standards rather than the library's. Nothing here
// ships: a wrong line on a terminal is the worst it can produce.
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
        // The variant, separately from the format code, because for one format they differ:
        // an unterminated value comes back as text or as bytes depending on whether it
        // decoded, and seeing which is the point of running this against a real file.
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
            // The value's own bytes, which for a terminated string is one less than the
            // `length` the index states - the terminator belongs to the format.
            entry.value.as_bytes().map_or(4, <[u8]>::len),
            entry.reserved,
            read_as
        );
    }

    println!();
    for key in &wanted {
        match sfo.bytes(key) {
            // Hex rather than a number, and no byte order chosen: which end of a user id is
            // significant is the consumer's question, and a probe that picked one would be
            // making the decision on their behalf in a place they would never look.
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
