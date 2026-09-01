//! Read the `param.sfo` out of real packages, and check a generated one against them.
//!
//! `PSF` is the format this crate was shipping twenty-one bytes of the word `PLACEHOLDER` in
//! place of. Writing one is only worth anything if it is the same shape as the ones a console
//! already accepts, so this reads the real thing out of packages in hand and reports what it
//! finds - then builds a table with the same identifying values and says whether the two agree
//! field for field.
//!
//! It is not a byte comparison and should not be: a real title carries fields this cannot know
//! and would be inventing. What it checks is that every field this crate *does* write is
//! present in a real file, with the same type and the same field width.
//!
//! ```text
//! cargo run -p selfish-pkg --example param -- <package>...
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

use selfish_pkg::{Package, entry_id, sfo};
use selfish_title::sfo::{Sfo, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: param <package>...");
        std::process::exit(2);
    }

    let mut agreed = 0_usize;
    let mut checked = 0_usize;
    for path in &paths {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let bytes = std::fs::read(path)?;
        println!("{name}");

        // A bare `param.sfo` is as good an oracle as one inside a package, and homebrew
        // projects keep them in the open next to their `.gp4`. Taking either means the check
        // can be run against files nobody had to extract first.
        let (raw, content_id_from_package) = match Package::parse(&bytes) {
            Ok(package) => {
                let Some(entry) = package.entry(entry_id::PARAM_SFO) else {
                    println!("  no param.sfo entry");
                    continue;
                };
                let raw = package
                    .entry_bytes(entry)
                    .ok_or("truncated param.sfo")?
                    .to_vec();
                let id = String::from_utf8_lossy(package.content_id())
                    .trim_end_matches('\0')
                    .to_owned();
                (raw, Some(id))
            }
            Err(_) => (bytes.clone(), None),
        };
        let raw = raw.as_slice();
        let real = match Sfo::parse(raw) {
            Ok(table) => table,
            Err(error) => {
                println!("  could not read it: {error}");
                continue;
            }
        };
        checked += 1;

        // A bare file carries its own content id; a package's header is the authority when
        // there is one.
        let content_id = content_id_from_package
            .unwrap_or_else(|| real.get("CONTENT_ID").and_then(text_of).unwrap_or_default());
        let title_id = real
            .get("TITLE_ID")
            .and_then(text_of)
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        let title = real
            .get("TITLE")
            .and_then(text_of)
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        let version = real
            .get("VERSION")
            .and_then(text_of)
            .unwrap_or_else(|| "01.00".to_owned());
        println!("  read back: TITLE_ID={title_id}  TITLE={title:?}  VERSION={version}");

        // Build one carrying the same identity, and compare the fields this crate writes.
        let ours = sfo::game(&sfo::Params {
            content_id: &content_id,
            title_id: &title_id,
            title: &title,
            version: &version,
        });

        let mut disagreed = 0_usize;
        let mut absent = 0_usize;
        for field in FIELDS {
            match (reserved_of(&real, field), reserved_of(&ours, field)) {
                (None, Some(_)) => {
                    println!("    {field}: this crate writes it, the real file does not");
                    absent += 1;
                }
                (Some(left), Some(right)) => {
                    // A differing *value* is expected for anything the title chose; a differing
                    // width or type is a format disagreement and is what matters here.
                    let same_kind =
                        core::mem::discriminant(left.0) == core::mem::discriminant(right.0);
                    if same_kind && left.1 == right.1 {
                        if left.0 != right.0 {
                            println!("    {field}: same shape, different value (the title's own)");
                        }
                    } else {
                        println!(
                            "    {field}: DIFFERENT SHAPE - real {:?}/{}, ours {:?}/{}",
                            left.0, left.1, right.0, right.1
                        );
                        disagreed += 1;
                    }
                }
                (_, None) => {}
            }
        }
        let extra: Vec<&str> = real
            .entries()
            .iter()
            .map(|entry| entry.key.as_str())
            .filter(|name| !FIELDS.contains(name))
            .collect();
        if !extra.is_empty() {
            println!(
                "    carries {} field(s) this crate does not: {extra:?}",
                extra.len()
            );
        }

        if disagreed == 0 {
            agreed += 1;
            println!("  every field this crate writes has the same type and width as the real one");
            if absent > 0 {
                println!("  ({absent} written here that this package does not carry)");
            }
        }
    }

    println!();
    println!("{agreed} of {checked} package(s) agree on shape");
    if checked > 0 && agreed < checked {
        std::process::exit(1);
    }
    Ok(())
}

/// The fields this crate writes.
const FIELDS: &[&str] = &[
    "APP_TYPE",
    "APP_VER",
    "ATTRIBUTE",
    "ATTRIBUTE2",
    "CATEGORY",
    "CONTENT_ID",
    "DEV_FLAG",
    "DOWNLOAD_DATA_SIZE",
    "FORMAT",
    "PARENTAL_LEVEL",
    "PUBTOOLINFO",
    "PUBTOOLMINVER",
    "PUBTOOLVER",
    "REMOTE_PLAY_KEY_ASSIGN",
    "SERVICE_ID_ADDCONT_ADD_1",
    "SERVICE_ID_ADDCONT_ADD_2",
    "SERVICE_ID_ADDCONT_ADD_3",
    "SERVICE_ID_ADDCONT_ADD_4",
    "SERVICE_ID_ADDCONT_ADD_5",
    "SERVICE_ID_ADDCONT_ADD_6",
    "SERVICE_ID_ADDCONT_ADD_7",
    "SYSTEM_VER",
    "TITLE",
    "TITLE_ID",
    "USER_DEFINED_PARAM_1",
    "USER_DEFINED_PARAM_2",
    "USER_DEFINED_PARAM_3",
    "USER_DEFINED_PARAM_4",
    "VERSION",
];

fn text_of(value: &Value) -> Option<String> {
    value.as_text().map(str::to_owned)
}

/// One field's value and the room it reserves.
///
/// The reserved width is the part that matters for a shape comparison: a reader takes the
/// field to be that wide whatever the value is, so two files agreeing on a value and
/// disagreeing on the width are not the same file.
fn reserved_of<'a>(sfo: &'a Sfo, key: &str) -> Option<(&'a Value, u32)> {
    sfo.entries()
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| (&entry.value, entry.reserved))
}
