//! `title`: what a package, `PARAM.SFO` or `param.json` says about its title.

use std::path::Path;

use crate::Result;

/// `selfish title`. The input is recognised by content, because `PARAM.SFO` inside a package is
/// an entry with no file name to go by.
pub(crate) fn describe(path: &Path, round_trip: bool) -> Result {
    let bytes = std::fs::read(path)?;

    if let Ok(package) = selfish_pkg::Package::parse(&bytes) {
        let entry = package
            .entry(selfish_pkg::entry_id::PARAM_SFO)
            .ok_or("the package carries no PARAM.SFO entry")?;
        let raw = package
            .entry_bytes(entry)
            .ok_or("the PARAM.SFO entry runs past the end of the package")?;
        say!(
            "source     package entry {:#x}",
            selfish_pkg::entry_id::PARAM_SFO
        );
        show_sfo(&selfish_title::Sfo::parse(raw)?, round_trip.then_some(raw));
        return Ok(());
    }
    if let Ok(sfo) = selfish_title::Sfo::parse(&bytes) {
        say!("source     PARAM.SFO");
        show_sfo(&sfo, round_trip.then_some(bytes.as_slice()));
        return Ok(());
    }

    let param = selfish_title::Param::parse(&bytes)?;
    say!("source     param.json");
    say!("title id   {}", param.title_id().unwrap_or("<absent>"));
    say!("content id {}", param.content_id().unwrap_or("<absent>"));
    say!("name       {}", param.title_name().unwrap_or("<absent>"));
    say!(
        "language   {} ({} localised)",
        param.default_language().unwrap_or("<absent>"),
        param.languages().len()
    );
    if let Some(category) = param.category() {
        say!("category   {category}");
    } else {
        say!("category   <absent>");
    }
    Ok(())
}

/// Every entry, then, when asked, whether writing the parsed file back gives the same bytes.
fn show_sfo(sfo: &selfish_title::Sfo, original: Option<&[u8]>) {
    use selfish_title::sfo::Value;

    say!("entries    {}", sfo.entries().len());
    for entry in sfo.entries() {
        let value = match &entry.value {
            Value::Text(text) => text.clone(),
            Value::TextUnterminated(text) => format!("{text} (unterminated)"),
            Value::Integer(number) => format!("{number} ({number:#x})"),
            Value::Binary(bytes) => format!("<{} bytes, unterminated>", bytes.len()),
            Value::Unknown(code, bytes) => format!("<format {code:#06x}, {} bytes>", bytes.len()),
        };
        say!("  {:<20} {value}", entry.key);
    }

    let Some(original) = original else { return };
    let written = sfo.to_bytes();
    if written == original {
        say!("round trip identical, {} bytes", written.len());
    } else {
        let at = written
            .iter()
            .zip(original)
            .position(|(left, right)| left != right);
        say!(
            "round trip DIFFERS: wrote {} bytes against {}, first difference at {:?}",
            written.len(),
            original.len(),
            at
        );
    }
}
