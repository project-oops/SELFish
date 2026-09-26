//! `container` and `audit`: the signed-executable container.

use std::path::Path;

use crate::Result;

/// `selfish container`: the header, the entries, and where the executable sits.
pub(crate) fn describe(path: &Path) -> Result {
    let bytes = std::fs::read(path)?;
    let container = selfish_container::Container::parse(&bytes)?;
    say!("generation {}", container.generation());
    say!("entries    {}", container.entries().len());
    say!(
        "header     {:#x}, metadata {:#x}, stated size {:#x}",
        container.header_size(),
        container.meta_size(),
        container.stated_file_size()
    );
    for entry in container.entries() {
        say!(
            "  {}  segment {:>3}  {:#012x} {:>10} bytes",
            if entry.carries_segment_data() {
                "data  "
            } else {
                "digest"
            },
            entry.segment_index(),
            entry.offset,
            entry.filesz
        );
    }
    match container.inner_elf_header() {
        Ok(_) => say!("inner executable at {:#x}", container.inner_offset()),
        Err(error) => say!("inner executable: {error}"),
    }
    Ok(())
}

/// `selfish audit`: a real container's fixed header rows against the format table.
///
/// What kind of container this is prints before the row count: a container written from this
/// table matches every row whatever it is, so the kind has to be read first. (D092)
pub(crate) fn audit(file: &Path) -> Result {
    let bytes = std::fs::read(file)?;
    let result = match selfish_container::audit(&bytes) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("not a container this crate recognises: {error:?}");
            std::process::exit(1);
        }
    };

    say!("generation  {:?} (from the magic)", result.generation);
    match &result.declared {
        selfish_container::Declared::Ptype { value, known } => {
            let name = known.as_deref().unwrap_or("not a kind this table names");
            say!("kind        ptype {value:#x} - {name}");
        }
        selfish_container::Declared::Unreachable => {
            say!("kind        unknown - `ex_info` is not at `header_size - 0x70` in this file");
        }
    }
    if let Some(caveat) = result.declared.caveat() {
        say!("            {caveat}");
    }
    say!("");
    say!(
        "confirmed   {} of {} fixed header row(s) match the table",
        result.confirmed(),
        result.header.len()
    );
    say!("");
    show_rows(&result.header, 4);

    let differing = result.differing();
    say!("");
    if differing.is_empty() {
        say!("every fixed header row this file carries agrees with the table.");
    } else {
        say!(
            "{} row(s) differ from the previous-generation table.",
            differing.len()
        );
        say!("a difference is a finding, not a new fact: record it against a citable");
        say!("source for this generation, do not read a meaning off these bytes.");
        for row in differing {
            if !row.note.is_empty() {
                say!("  {} - the table's note: {}", row.field, row.note);
            }
        }
    }

    show_tail(&result);
    Ok(())
}

/// One line per row: matched or not, the field, its offset, the table's value and the file's.
fn show_rows(rows: &[selfish_container::RowVerdict], offset_width: usize) {
    for row in rows {
        let mark = if row.matched { "  ok  " } else { " DIFF " };
        let found = row.found.map_or_else(
            || "(past the dump)".to_owned(),
            |found| format!("{found:#x}"),
        );
        say!(
            "{mark} {:<14} @{:#0offset_width$x}  table {:#x}  file {found}",
            row.field,
            row.offset,
            row.expected,
        );
    }
}

/// The `ex_info` rows, reported apart from the header.
///
/// A header row differing is a claim about the format; a tail row differing usually means the
/// file is not a fake container, because the table's tail values come from a writer that made
/// only fake ones.
fn show_tail(result: &selfish_container::Audit) {
    say!("");
    let Some(first) = result.tail.first() else {
        say!("ex_info     not reachable at `header_size - 0x70`, so the tail was not checked.");
        return;
    };
    say!("ex_info     the tail, at {:#x}", first.offset);
    show_rows(&result.tail, 6);

    let differing = result.tail_differing();
    if !differing.is_empty() {
        say!("");
        say!(
            "{} tail row(s) differ. These are what a *fake* container carries, so this",
            differing.len()
        );
        say!("is expected of anything this project did not write - and it is not evidence");
        say!("that the layout is wrong. Read it with the `kind` line above.");
    }
}
