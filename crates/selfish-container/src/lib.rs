//! The signed-executable container, read and written.
//!
//! What the platform wraps an executable in. A retail one is signed with keys nobody outside
//! the vendor has; the ones built here declare themselves **fake** in the field the format
//! provides for exactly that, with every digest and the whole signature area left zero.
//!
//! # Both directions, in one place, on purpose
//!
//! A format library that only parses is half a library, and the half that writes is where
//! the errors are. Keeping them together makes the round trip a test - parse what was
//! written, write what was parsed, and fail if they differ - which is a far better guard
//! than either half can have alone.
//!
//! # The layout
//!
//! ```text
//! [header 32][entry 32 x N][ELF header + program headers][pad][ex_info 64][npdrm 48]  <- header_size
//! [meta_block 80 x N][meta_footer 80][signature 256]                                  <- meta_size
//! [payloads, each aligned]
//! ```
//!
//! **Two entries per segment, not one.** A digest entry carrying one digest per block, then
//! the segment itself. No reader states this, because no reader has to build one - it came
//! from an open-source *writer*, and it is the single thing most likely to be got wrong by
//! reading only loaders. A container whose entries are all zero-propped is structurally
//! valid and rejected inside the loader's segment walk, with no indication why.
//!
//! # Every constant comes from the table
//!
//! `data/self-format.tsv` carries the fields and the projects each was established from.
//! Nothing here holds a second copy. See [`table`].

#![forbid(unsafe_code)]

pub mod table;

use core::fmt;

use selfish_abi::Generation;
use selfish_elf::{Elf, ElfError};

/// Size of the container header.
pub const HEADER_SIZE: u64 = 32;

/// Size of one entry descriptor.
pub const ENTRY_SIZE: u64 = 32;

/// Size of one metadata block.
pub const META_BLOCK_SIZE: u64 = 80;

/// Size of the metadata footer.
pub const META_FOOTER_SIZE: u64 = 80;

/// Size of the extended info block.
pub const EX_INFO_SIZE: u64 = 64;

/// Every constant the container needs, read once so a missing one fails before any byte is
/// written rather than halfway through.
#[derive(Debug, Clone)]
struct Constants {
    key_type: u32,
    header_flags: u16,
    digest_size: u64,
    signature_size: u64,
    block_size: u64,
    npdrm_block_type: u16,
    content_id_size: u64,
    random_pad_size: u64,
    align: u64,
    ptype_fake: u64,
    paid: u64,
    footer_unk1: u32,
    version: u8,
    mode: u8,
    endian: u8,
    attributes: u8,
    signed_shift: u32,
    has_blocks_shift: u32,
    block_size_shift: u32,
    has_digests_shift: u32,
    segment_index_shift: u32,
}

impl Constants {
    fn load() -> Result<Self, ContainerError> {
        let need = |group: &str, field: &str| -> Result<u64, ContainerError> {
            table::lookup(group, field).ok_or_else(|| ContainerError::MissingConstant {
                group: group.to_owned(),
                field: field.to_owned(),
            })
        };
        let small = |group: &str, field: &str| -> Result<u8, ContainerError> {
            u8::try_from(need(group, field)?).map_err(|_| ContainerError::MissingConstant {
                group: group.to_owned(),
                field: field.to_owned(),
            })
        };
        let shift = |field: &str| -> Result<u32, ContainerError> {
            u32::try_from(need("entry_prop", field)?).map_err(|_| ContainerError::MissingConstant {
                group: "entry_prop".to_owned(),
                field: field.to_owned(),
            })
        };
        Ok(Self {
            key_type: u32::try_from(need("self_header", "key_type")?).unwrap_or(0),
            header_flags: u16::try_from(need("self_header", "flags")?).unwrap_or(0),
            digest_size: need("const", "digest_size")?,
            signature_size: need("const", "signature_size")?,
            block_size: need("const", "block_size")?,
            npdrm_block_type: u16::try_from(need("const", "npdrm_block_type")?).unwrap_or(0),
            content_id_size: need("const", "content_id_size")?,
            random_pad_size: need("const", "random_pad_size")?,
            align: need("const", "header_align")?,
            ptype_fake: need("ptype", "fake")?,
            paid: need("ex_info", "paid")?,
            footer_unk1: u32::try_from(need("meta_footer", "unk1")?).unwrap_or(0),
            version: small("self_header", "version")?,
            mode: small("self_header", "mode")?,
            endian: small("self_header", "endian")?,
            attributes: small("self_header", "attributes")?,
            signed_shift: shift("signed_shift")?,
            has_blocks_shift: shift("has_blocks_shift")?,
            block_size_shift: shift("block_size_shift")?,
            has_digests_shift: shift("has_digests_shift")?,
            segment_index_shift: shift("segment_index_shift")?,
        })
    }
}

/// Program header types that become container entries, read from the table so the list
/// cannot drift from the record of where it came from.
#[must_use]
pub fn entry_segment_types() -> Vec<u32> {
    table::group("phdr_type")
        .into_iter()
        .filter(|(field, _)| {
            table::note("phdr_type", field)
                .unwrap_or_default()
                .contains("becomes two entries")
        })
        .filter_map(|(_, value)| u32::try_from(value).ok())
        .collect()
}

/// One entry descriptor, as it appears on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// Properties: the flag bits and the segment index.
    pub props: u64,
    /// Where the described data begins.
    pub offset: u64,
    /// Bytes present.
    pub filesz: u64,
    /// Bytes when expanded.
    pub memsz: u64,
}

impl Entry {
    /// Whether this entry describes segment data a loader should map.
    ///
    /// The bit a loader searches on. A container where no entry carries it parses perfectly
    /// and then falls off the end of the segment walk with nothing to say about why.
    ///
    /// Reads its own shift from the table rather than taking one. The first version of this
    /// took a `Constants`, which is private - so no consumer of this crate could have called
    /// the one method that answers the question they actually have.
    #[must_use]
    pub fn carries_segment_data(&self) -> bool {
        table::lookup("entry_prop", "has_blocks_shift")
            .and_then(|s| u32::try_from(s).ok())
            .is_some_and(|shift| self.props & bit(shift) != 0)
    }

    /// Which program header this entry belongs to.
    ///
    /// Meaningful only for an entry that [carries segment data](Self::carries_segment_data);
    /// a digest entry uses the same field to point at the data entry that follows it.
    #[must_use]
    pub fn segment_index(&self) -> u32 {
        table::lookup("entry_prop", "segment_index_shift")
            .and_then(|s| u32::try_from(s).ok())
            .and_then(|shift| u32::try_from((self.props >> shift) & 0xFFFF).ok())
            .unwrap_or(0)
    }
}

/// A parsed container.
#[derive(Debug)]
pub struct Container<'a> {
    bytes: &'a [u8],
    generation: Generation,
    header_size: u64,
    meta_size: u64,
    file_size: u64,
    entries: Vec<Entry>,
}

impl<'a> Container<'a> {
    /// Parse a container.
    ///
    /// # Errors
    ///
    /// If the magic matches neither generation, or the entry table runs past the end.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ContainerError> {
        let head = bytes
            .get(..usize::try_from(HEADER_SIZE).unwrap_or(32))
            .ok_or(ContainerError::TooShort)?;
        let mut magic = [0_u8; 4];
        magic.copy_from_slice(head.get(..4).ok_or(ContainerError::TooShort)?);
        let generation =
            Generation::from_container_magic(magic).ok_or(ContainerError::NotAContainer(magic))?;

        let header_size = u64::from(read_u16(head, 0x0C)?);
        let meta_size = u64::from(read_u16(head, 0x0E)?);
        let file_size = read_u64(head, 0x10)?;
        let count = read_u16(head, 0x18)?;

        let mut entries = Vec::with_capacity(usize::from(count));
        for index in 0..u64::from(count) {
            let at = usize::try_from(HEADER_SIZE.saturating_add(index.saturating_mul(ENTRY_SIZE)))
                .map_err(|_| ContainerError::EntriesOutOfBounds)?;
            let end = at
                .checked_add(usize::try_from(ENTRY_SIZE).unwrap_or(32))
                .ok_or(ContainerError::EntriesOutOfBounds)?;
            let raw = bytes
                .get(at..end)
                .ok_or(ContainerError::EntriesOutOfBounds)?;
            entries.push(Entry {
                props: read_u64(raw, 0)?,
                offset: read_u64(raw, 8)?,
                filesz: read_u64(raw, 16)?,
                memsz: read_u64(raw, 24)?,
            });
        }

        Ok(Self {
            bytes,
            generation,
            header_size,
            meta_size,
            file_size,
            entries,
        })
    }

    /// Which console this container is for.
    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// The entry descriptors.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Where the payload begins, per the header.
    #[must_use]
    pub const fn header_size(&self) -> u64 {
        self.header_size
    }

    /// Size of the metadata region, per the header.
    #[must_use]
    pub const fn meta_size(&self) -> u64 {
        self.meta_size
    }

    /// The size the header claims the whole file is.
    #[must_use]
    pub const fn stated_file_size(&self) -> u64 {
        self.file_size
    }

    /// Where the inner executable's headers begin.
    ///
    /// Derived rather than assumed: immediately after the entry table. An observed constant
    /// would silently mis-parse anything with a different entry count.
    #[must_use]
    pub fn inner_offset(&self) -> u64 {
        HEADER_SIZE.saturating_add(
            u64::try_from(self.entries.len())
                .unwrap_or(0)
                .saturating_mul(ENTRY_SIZE),
        )
    }

    /// The inner executable's header bytes, checked for ELF magic.
    ///
    /// # Errors
    ///
    /// If the derived offset does not hold an executable, which means the derivation is
    /// wrong rather than that the file is merely unusual.
    pub fn inner_elf_header(&self) -> Result<&'a [u8], ContainerError> {
        let at = usize::try_from(self.inner_offset()).map_err(|_| ContainerError::TooShort)?;
        let rest = self.bytes.get(at..).ok_or(ContainerError::TooShort)?;
        if rest.get(..4) != Some(&selfish_elf::MAGIC) {
            return Err(ContainerError::NoInnerElf(self.inner_offset()));
        }
        Ok(rest)
    }

    /// Reassemble the executable this container holds.
    ///
    /// The inverse of [`build`]. A container does not store the executable contiguously: its
    /// headers sit after the entry table, and each segment's contents live wherever an entry
    /// says. Putting it back means writing every segment to the file offset its *program
    /// header* names, which is the layout an ordinary reader expects.
    ///
    /// Only entries carrying segment data contribute. The digest entries beside them describe
    /// the same segment and hold no contents.
    ///
    /// # Errors
    ///
    /// If the inner headers are absent or malformed, or an entry points outside the file.
    pub fn to_elf(&self) -> Result<Vec<u8>, ContainerError> {
        let inner = self.inner_elf_header()?;
        let elf = Elf::parse(inner)?;
        let span = usize::try_from(elf.header_span())
            .map_err(|_| ContainerError::Arithmetic("header span"))?;

        // Sized by the furthest any program header reaches, not by the container's own
        // length: the container is larger, and the difference is metadata that has no place
        // in a reassembled executable.
        let mut end = span;
        for phdr in elf.program_headers() {
            let reach = usize::try_from(phdr.offset.get())
                .ok()
                .and_then(|o| o.checked_add(usize::try_from(phdr.filesz.get()).ok()?))
                .ok_or(ContainerError::Arithmetic("segment reach"))?;
            if reach > end {
                end = reach;
            }
        }

        let mut out = vec![0_u8; end];
        let head = inner
            .get(..span)
            .ok_or(ContainerError::Arithmetic("inner headers"))?;
        out.get_mut(..span)
            .ok_or(ContainerError::Arithmetic("inner headers"))?
            .copy_from_slice(head);

        for entry in &self.entries {
            if !entry.carries_segment_data() {
                continue;
            }
            let index = usize::try_from(entry.segment_index())
                .map_err(|_| ContainerError::Arithmetic("segment index"))?;
            let Some(phdr) = elf.program_headers().get(index) else {
                // An index past the program header table is a container describing a segment
                // the executable does not have. Skipped rather than fatal: the rest is still
                // recoverable, and a partial executable beats none.
                continue;
            };
            let from = usize::try_from(entry.offset)
                .map_err(|_| ContainerError::Arithmetic("entry offset"))?;
            let len = usize::try_from(entry.filesz)
                .map_err(|_| ContainerError::Arithmetic("entry size"))?;
            let source = self
                .bytes
                .get(
                    from..from
                        .checked_add(len)
                        .ok_or(ContainerError::Arithmetic("entry span"))?,
                )
                .ok_or(ContainerError::Arithmetic("an entry points past the end"))?;
            let to = usize::try_from(phdr.offset.get())
                .map_err(|_| ContainerError::Arithmetic("segment offset"))?;
            let slot = out
                .get_mut(
                    to..to
                        .checked_add(len)
                        .ok_or(ContainerError::Arithmetic("segment span"))?,
                )
                .ok_or(ContainerError::Arithmetic("a segment lands past the end"))?;
            slot.copy_from_slice(source);
        }
        Ok(out)
    }
}

/// One props bit.
fn bit(shift: u32) -> u64 {
    1_u64.checked_shl(shift).unwrap_or(0)
}

/// A props field at its shift.
fn field(shift: u32, value: u64) -> u64 {
    value.checked_shl(shift).unwrap_or(0)
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, ContainerError> {
    let end = at.checked_add(2).ok_or(ContainerError::TooShort)?;
    let raw = bytes.get(at..end).ok_or(ContainerError::TooShort)?;
    let mut out = [0_u8; 2];
    out.copy_from_slice(raw);
    Ok(u16::from_le_bytes(out))
}

fn read_u64(bytes: &[u8], at: usize) -> Result<u64, ContainerError> {
    let end = at.checked_add(8).ok_or(ContainerError::TooShort)?;
    let raw = bytes.get(at..end).ok_or(ContainerError::TooShort)?;
    let mut out = [0_u8; 8];
    out.copy_from_slice(raw);
    Ok(u64::from_le_bytes(out))
}

/// Round `value` up to a multiple of `to`, refusing rather than wrapping.
fn align_up(value: u64, to: u64) -> Option<u64> {
    if to == 0 {
        return None;
    }
    value
        .checked_add(to.checked_sub(1)?)?
        .checked_div(to)?
        .checked_mul(to)
}

/// Block size is stored as an exponent: `log2(bytes) - 12`.
///
/// So 16KiB is written as 2. Storing the byte count fits the four-bit field for small values
/// and truncates for real ones - a container a loader accepts and then reads from the wrong
/// place.
fn block_size_code(bytes: u64) -> Result<u64, ContainerError> {
    if !bytes.is_power_of_two() {
        return Err(ContainerError::Arithmetic(
            "block size is not a power of two",
        ));
    }
    u64::from(bytes.trailing_zeros())
        .checked_sub(12)
        .ok_or(ContainerError::Arithmetic("block size is below the floor"))
}

/// Bytes accumulated in order, so no offset is written by hand.
#[derive(Debug, Default)]
struct Sink {
    bytes: Vec<u8>,
}

impl Sink {
    fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn raw(&mut self, v: &[u8]) {
        self.bytes.extend_from_slice(v);
    }
    fn zeros(&mut self, n: usize) {
        self.bytes.resize(self.bytes.len().saturating_add(n), 0);
    }
    fn pad_to(&mut self, offset: usize) {
        if offset > self.bytes.len() {
            self.zeros(offset.saturating_sub(self.bytes.len()));
        }
    }
}

/// What a real container says about one row the table pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowVerdict {
    /// The field, as the table names it.
    pub field: String,
    /// Where the table places it.
    pub offset: usize,
    /// The value the table claims.
    pub expected: u64,
    /// What the real file holds there, if it could be read.
    pub found: Option<u64>,
    /// Whether the two agree.
    pub matched: bool,
    /// The table's own note on the row.
    pub note: String,
}

/// The result of checking a real container against the table.
///
/// # Why this exists, and the line it must not cross
///
/// `data/self-format.tsv` is derived from cited **previous-generation** sources and says so:
/// every row is a hypothesis until a current-generation file accepts or rejects it. This is the
/// oracle step the charter describes (principle 2) - derive from something citable, then check
/// against reality, and record which rows reality settled.
///
/// It reports agreement and disagreement. It does **not** interpret a disagreement: a row a
/// real file contradicts is recorded as differing, with the real value beside the expected one,
/// and settling what the field means at the new generation needs a citable source, not this
/// binary (principle 1). A difference is a finding, not a derivation.
#[derive(Debug, Clone)]
pub struct Audit {
    /// The generation the magic identifies.
    pub generation: Generation,
    /// One verdict per fixed row in the header.
    pub header: Vec<RowVerdict>,
}

impl Audit {
    /// How many rows the file confirmed.
    #[must_use]
    pub fn confirmed(&self) -> usize {
        self.header.iter().filter(|row| row.matched).count()
    }

    /// The rows the file contradicted - the ones a new generation may have changed.
    #[must_use]
    pub fn differing(&self) -> Vec<&RowVerdict> {
        self.header.iter().filter(|row| !row.matched).collect()
    }
}

/// Check a real container against the format table.
///
/// The magic is read to identify the generation and is then **not** counted as a mismatch:
/// the magic differing across generations is the generation split itself (D003), not a row the
/// newer file got wrong. Every other fixed row in the header is a claim the table makes that
/// this file settles.
///
/// # Errors
///
/// If the bytes are too short to hold a header, or the magic is not a container's.
pub fn audit(bytes: &[u8]) -> Result<Audit, ContainerError> {
    let container = Container::parse(bytes)?;
    let generation = container.generation();

    let mut header = Vec::new();
    for row in table::fixed_fields("self_header") {
        // The magic is the generation marker, read above. Confirming it against a
        // previous-generation value would report the whole point of the split as an error.
        if row.field == "magic" {
            continue;
        }
        let found = read_at(bytes, row.offset, row.size);
        header.push(RowVerdict {
            matched: found == Some(row.value),
            field: row.field,
            offset: row.offset,
            expected: row.value,
            found,
            note: row.note,
        });
    }
    Ok(Audit { generation, header })
}

/// Read a little-endian value of `size` bytes at `offset`, if it fits.
fn read_at(bytes: &[u8], offset: usize, size: usize) -> Option<u64> {
    if size == 0 || size > 8 {
        return None;
    }
    let slice = bytes.get(offset..offset.checked_add(size)?)?;
    let mut value = 0_u64;
    for (index, byte) in slice.iter().enumerate() {
        let shift = u32::try_from(index).ok()?.checked_mul(8)?;
        value |= u64::from(*byte).checked_shl(shift)?;
    }
    Some(value)
}

/// Wrap an executable in a fake container for a given generation.
///
/// # Errors
///
/// If the payload is not a usable executable, no segment qualifies as an entry, a size
/// computation overflows, or the format table is missing a constant.
pub fn build(payload: &[u8], generation: Generation) -> Result<Vec<u8>, ContainerError> {
    let constants = Constants::load()?;
    let elf = Elf::parse(payload)?;
    let types = entry_segment_types();

    let chosen: Vec<_> = elf
        .program_headers()
        .iter()
        .enumerate()
        .filter(|(_, p)| types.contains(&p.p_type.get()))
        .collect();
    if chosen.is_empty() {
        return Err(ContainerError::NoSegments);
    }

    let entry_count = u64::try_from(chosen.len())
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or(ContainerError::Arithmetic("entry count"))?;
    let ehdr_span = elf.header_span();

    let header_size = header_size_for(&constants, entry_count, ehdr_span)?;
    let meta_size = entry_count
        .checked_mul(META_BLOCK_SIZE)
        .and_then(|n| n.checked_add(META_FOOTER_SIZE))
        .and_then(|n| n.checked_add(constants.signature_size))
        .ok_or(ContainerError::Arithmetic("metadata size"))?;

    let entries = plan_entries(&constants, &chosen, header_size, meta_size)?;
    let total = entries
        .last()
        .and_then(|e| e.offset.checked_add(e.filesz))
        .and_then(|n| align_up(n, constants.align))
        .ok_or(ContainerError::Arithmetic("container end"))?;

    let mut out = Sink::default();
    write_header(
        &mut out,
        &constants,
        generation,
        entry_count,
        header_size,
        meta_size,
        total,
    )?;
    for entry in &entries {
        out.u64(entry.props);
        out.u64(entry.offset);
        out.u64(entry.filesz);
        out.u64(entry.memsz);
    }
    let span = usize::try_from(ehdr_span).map_err(|_| ContainerError::Arithmetic("header span"))?;
    out.raw(payload.get(..span).ok_or(ContainerError::Arithmetic(
        "payload shorter than its headers",
    ))?);
    out.pad_to(prefix_end(&constants, entry_count, ehdr_span)?);
    write_ex_info(&mut out, &constants);
    write_npdrm(&mut out, &constants);
    out.pad_to(usize::try_from(header_size).map_err(|_| ContainerError::Arithmetic("header"))?);
    write_metadata(&mut out, &constants, entry_count)?;
    write_payloads(&mut out, payload, &chosen, &entries)?;
    out.pad_to(usize::try_from(total).map_err(|_| ContainerError::Arithmetic("total"))?);
    Ok(out.bytes)
}

/// Everything before `ex_info`, aligned.
fn prefix_end(
    constants: &Constants,
    entry_count: u64,
    ehdr_span: u64,
) -> Result<usize, ContainerError> {
    let before = HEADER_SIZE
        .checked_add(entry_count.saturating_mul(ENTRY_SIZE))
        .and_then(|n| n.checked_add(ehdr_span))
        .ok_or(ContainerError::Arithmetic("header prefix"))?;
    let aligned =
        align_up(before, constants.align).ok_or(ContainerError::Arithmetic("header align"))?;
    usize::try_from(aligned).map_err(|_| ContainerError::Arithmetic("header align"))
}

fn header_size_for(
    constants: &Constants,
    entry_count: u64,
    ehdr_span: u64,
) -> Result<u64, ContainerError> {
    let aligned = u64::try_from(prefix_end(constants, entry_count, ehdr_span)?)
        .map_err(|_| ContainerError::Arithmetic("header align"))?;
    let npdrm = 2_u64
        .checked_add(14)
        .and_then(|n| n.checked_add(constants.content_id_size))
        .and_then(|n| n.checked_add(constants.random_pad_size))
        .ok_or(ContainerError::Arithmetic("npdrm block"))?;
    aligned
        .checked_add(EX_INFO_SIZE)
        .and_then(|n| n.checked_add(npdrm))
        .ok_or(ContainerError::Arithmetic("header size"))
}

/// Two entries per segment, at the offsets they will actually occupy.
///
/// Computed once and read twice - from the entry table and from the payload writer - rather
/// than computed twice and hoped to agree.
fn plan_entries(
    constants: &Constants,
    chosen: &[(usize, &selfish_elf::RawProgramHeader)],
    header_size: u64,
    meta_size: u64,
) -> Result<Vec<Entry>, ContainerError> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut offset = header_size
        .checked_add(meta_size)
        .ok_or(ContainerError::Arithmetic("first payload offset"))?;

    for (phdr_index, phdr) in chosen {
        let filesz = phdr.filesz.get();
        let blocks = align_up(filesz, constants.block_size)
            .and_then(|n| n.checked_div(constants.block_size))
            .ok_or(ContainerError::Arithmetic("block count"))?;
        let digest_bytes = blocks
            .checked_mul(constants.digest_size)
            .ok_or(ContainerError::Arithmetic("digest area"))?;

        // The digest entry points at the data entry that follows it.
        let next = u64::try_from(entries.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or(ContainerError::Arithmetic("entry index"))?;
        entries.push(Entry {
            props: bit(constants.signed_shift)
                | bit(constants.has_digests_shift)
                | field(constants.segment_index_shift, next),
            offset,
            filesz: digest_bytes,
            memsz: digest_bytes,
        });
        offset = offset
            .checked_add(digest_bytes)
            .and_then(|n| align_up(n, constants.align))
            .ok_or(ContainerError::Arithmetic("offset after digests"))?;

        // The data entry is what a loader searches for, and its index must be the *program
        // header's*, not this entry's position.
        entries.push(Entry {
            props: bit(constants.signed_shift)
                | bit(constants.has_blocks_shift)
                | field(
                    constants.block_size_shift,
                    block_size_code(constants.block_size)?,
                )
                | field(
                    constants.segment_index_shift,
                    u64::try_from(*phdr_index).unwrap_or(0),
                ),
            offset,
            filesz,
            // The *data's* uncompressed size, which is not the segment's memory size.
            //
            // Nothing here is compressed, so it is `filesz`. It read `p_memsz`, and for an
            // ordinary `PT_LOAD` the two agree often enough that the mistake stayed invisible
            // - until a segment where they differ.
            //
            // Two in a real executable differ, and both settle it the same way: a `PT_LOAD`
            // with `p_filesz 0x130` and `p_memsz 0x240` (bss) has an entry of `0x130`, and the
            // `PT_SCE_DYNLIBDATA` segment, whose `p_memsz` is **zero** because it is never
            // mapped, has an entry of `0x3760` - its full size on disk.
            //
            // The zero is what made this fatal rather than untidy. The authentication manager
            // divides this field by the block size to decide how many blocks to load, so a
            // segment with `0xb0` bytes in it came out as *zero blocks*: `sz for b error`,
            // four decrypt retries, `failed to load block`, and - several layers up, naming a
            // structure nobody had yet read - `Failed to load SCE_DYNLIBDATA: 5`. (D075)
            memsz: filesz,
        });
        offset = offset
            .checked_add(filesz)
            .and_then(|n| align_up(n, constants.align))
            .ok_or(ContainerError::Arithmetic("offset after segment"))?;
    }
    Ok(entries)
}

fn write_header(
    out: &mut Sink,
    constants: &Constants,
    generation: Generation,
    entry_count: u64,
    header_size: u64,
    meta_size: u64,
    file_size: u64,
) -> Result<(), ContainerError> {
    out.raw(&generation.container_magic());
    out.u8(constants.version);
    out.u8(constants.mode);
    out.u8(constants.endian);
    out.u8(constants.attributes);
    out.u32(constants.key_type);
    out.u16(u16::try_from(header_size).map_err(|_| ContainerError::Arithmetic("header size"))?);
    out.u16(u16::try_from(meta_size).map_err(|_| ContainerError::Arithmetic("meta size"))?);
    out.u64(file_size);
    out.u16(u16::try_from(entry_count).map_err(|_| ContainerError::Arithmetic("entry count"))?);
    out.u16(constants.header_flags);
    out.u32(0);
    Ok(())
}

/// `ptype` is the whole mechanism: it says *fake* in a field rather than pretending.
fn write_ex_info(out: &mut Sink, constants: &Constants) {
    out.u64(constants.paid);
    out.u64(constants.ptype_fake);
    out.u64(0);
    out.u64(0);
    out.zeros(32);
}

fn write_npdrm(out: &mut Sink, constants: &Constants) {
    out.u16(constants.npdrm_block_type);
    out.zeros(14);
    out.zeros(usize::try_from(constants.content_id_size).unwrap_or(0));
    out.zeros(usize::try_from(constants.random_pad_size).unwrap_or(0));
}

/// One block per entry, a footer, and the signature area - all zero but one constant.
///
/// Not laziness. It is the same statement `ptype` makes: a digest area filled with anything
/// else would be a claim about content nothing computed, and a signature area filled with
/// anything would be a forgery rather than a blank.
fn write_metadata(
    out: &mut Sink,
    constants: &Constants,
    entry_count: u64,
) -> Result<(), ContainerError> {
    out.zeros(
        usize::try_from(entry_count.saturating_mul(META_BLOCK_SIZE))
            .map_err(|_| ContainerError::Arithmetic("metadata blocks"))?,
    );
    out.zeros(48);
    out.u32(constants.footer_unk1);
    out.zeros(28);
    out.zeros(
        usize::try_from(constants.signature_size)
            .map_err(|_| ContainerError::Arithmetic("signature"))?,
    );
    Ok(())
}

fn write_payloads(
    out: &mut Sink,
    payload: &[u8],
    chosen: &[(usize, &selfish_elf::RawProgramHeader)],
    entries: &[Entry],
) -> Result<(), ContainerError> {
    for (index, (_, phdr)) in chosen.iter().enumerate() {
        let digest = entries
            .get(index.saturating_mul(2))
            .ok_or(ContainerError::Arithmetic("digest entry"))?;
        out.pad_to(usize::try_from(digest.offset).unwrap_or(usize::MAX));
        out.zeros(usize::try_from(digest.filesz).unwrap_or(0));

        let data = entries
            .get(index.saturating_mul(2).saturating_add(1))
            .ok_or(ContainerError::Arithmetic("data entry"))?;
        out.pad_to(usize::try_from(data.offset).unwrap_or(usize::MAX));
        let start = usize::try_from(phdr.offset.get()).unwrap_or(usize::MAX);
        let len = usize::try_from(phdr.filesz.get()).unwrap_or(0);
        let end = start
            .checked_add(len)
            .ok_or(ContainerError::Arithmetic("segment span"))?;
        out.raw(payload.get(start..end).ok_or(ContainerError::Arithmetic(
            "a program header points past the end of the payload",
        ))?);
    }
    Ok(())
}

/// Why a container could not be read or built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerError {
    /// The format table has no such row.
    MissingConstant {
        /// The group.
        group: String,
        /// The field.
        field: String,
    },
    /// Shorter than a header.
    TooShort,
    /// The magic matches neither generation.
    ///
    /// Carries what was found, because "not a container" sends somebody looking in the wrong
    /// place when the answer is usually "this is a plain executable".
    NotAContainer([u8; 4]),
    /// The entry table runs past the end of the file.
    EntriesOutOfBounds,
    /// No executable at the derived inner offset, so the derivation is wrong.
    NoInnerElf(u64),
    /// The payload is not a usable executable.
    Elf(ElfError),
    /// A size or offset computation failed.
    Arithmetic(&'static str),
    /// No program header qualifies as an entry.
    NoSegments,
}

impl fmt::Display for ContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConstant { group, field } => write!(
                f,
                "the format table has no `{group}` / `{field}` row; the table and this code \
                 have diverged"
            ),
            Self::TooShort => write!(f, "shorter than a container header"),
            Self::NotAContainer(found) => write!(
                f,
                "not a container: begins {:02x} {:02x} {:02x} {:02x}",
                found.first().copied().unwrap_or(0),
                found.get(1).copied().unwrap_or(0),
                found.get(2).copied().unwrap_or(0),
                found.get(3).copied().unwrap_or(0)
            ),
            Self::EntriesOutOfBounds => {
                write!(f, "the entry table runs past the end of the file")
            }
            Self::NoInnerElf(at) => write!(
                f,
                "no executable at {at:#x}, where the entry count says one should begin"
            ),
            Self::Elf(e) => write!(f, "the payload is not a usable executable: {e}"),
            Self::Arithmetic(what) => write!(f, "size computation failed: {what}"),
            Self::NoSegments => write!(
                f,
                "no program header qualifies as an entry, so the container would describe \
                 nothing"
            ),
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<ElfError> for ContainerError {
    fn from(e: ElfError) -> Self {
        Self::Elf(e)
    }
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    reason = "fixture builders read better indexed, and a panic here is the test failing"
)]
mod tests {
    use super::{Constants, Container, ContainerError, Entry, audit, build, entry_segment_types};
    use selfish_abi::Generation;
    use selfish_elf::{ObjectType, segment};

    /// A minimal executable with one loadable segment carrying recognisable bytes.
    fn payload() -> Vec<u8> {
        const HEADER: usize = 64;
        const PHDR: usize = 56;
        let content: Vec<u8> = (0..=255_u8).cycle().take(300).collect();
        let seg_offset = HEADER + PHDR;
        let mut out = vec![0_u8; seg_offset + content.len()];
        out[..4].copy_from_slice(&selfish_elf::MAGIC);
        out[selfish_elf::EI_CLASS] = selfish_elf::CLASS64;
        out[selfish_elf::EI_DATA] = selfish_elf::DATA_LSB;
        out[selfish_elf::EI_OSABI] = selfish_elf::OSABI_FREEBSD;
        out[selfish_elf::EI_ABIVERSION] = 2;
        out[16..18].copy_from_slice(&ObjectType::EXECUTABLE.to_le_bytes());
        out[18..20].copy_from_slice(&selfish_elf::MACHINE_X86_64.to_le_bytes());
        out[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
        out[52..54].copy_from_slice(&(HEADER as u16).to_le_bytes());
        out[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes());
        out[56..58].copy_from_slice(&1_u16.to_le_bytes());
        out[64..68].copy_from_slice(&segment::LOAD.to_le_bytes());
        out[72..80].copy_from_slice(&(seg_offset as u64).to_le_bytes());
        out[96..104].copy_from_slice(&(content.len() as u64).to_le_bytes());
        out[104..112].copy_from_slice(&(content.len() as u64).to_le_bytes());
        out[seg_offset..].copy_from_slice(&content);
        out
    }

    #[test]
    fn every_constant_the_builder_needs_is_in_the_table() {
        Constants::load().expect("the format table is missing a constant this code needs");
    }

    #[test]
    fn four_segment_types_become_entries() {
        let types = entry_segment_types();
        assert_eq!(types.len(), 4, "LOAD, RELRO, DYNLIBDATA, COMMENT");
        assert!(types.contains(&segment::LOAD));
        assert!(types.contains(&segment::SCE_DYNLIBDATA));
    }

    #[test]
    fn a_container_round_trips_through_its_own_parser() {
        // The reason both directions live in one crate. A writer checked only against
        // itself is checked against nothing.
        for generation in [Generation::Current, Generation::Previous] {
            let built = build(&payload(), generation).expect("builds");
            let parsed = Container::parse(&built).expect("parses");
            assert_eq!(parsed.generation(), generation);
            assert_eq!(parsed.entries().len(), 2, "two entries per segment");
            assert_eq!(
                parsed.stated_file_size(),
                built.len() as u64,
                "the stated size should be the real one"
            );
            parsed
                .inner_elf_header()
                .expect("the inner executable is where the entry count says");
        }
    }

    #[test]
    fn the_data_entry_carries_the_bit_a_loader_searches_for() {
        // A container with all-zero props is structurally valid and dies inside a loader's
        // segment walk with no indication why. This is that bug, as a test.
        let built = build(&payload(), Generation::Current).expect("builds");
        let parsed = Container::parse(&built).expect("parses");
        assert!(
            parsed.entries().iter().any(Entry::carries_segment_data),
            "no entry carries the segment-data bit; a loader would find nothing to map"
        );
    }

    #[test]
    fn the_data_entry_points_at_its_program_header_not_at_itself() {
        let built = build(&payload(), Generation::Current).expect("builds");
        let parsed = Container::parse(&built).expect("parses");
        let data = parsed
            .entries()
            .iter()
            .find(|e| e.carries_segment_data())
            .expect("a data entry");
        assert_eq!(
            data.segment_index(),
            0,
            "the only qualifying program header is index 0"
        );
    }

    #[test]
    fn a_data_entry_sizes_itself_by_its_data_and_never_by_p_memsz() {
        // The block layer divides `memsz` by the block size to decide how many blocks to
        // load. A segment that is never mapped declares `p_memsz` zero, and copying that
        // here asks for zero blocks of a segment that has bytes in it - which the
        // authentication manager rejects before it decrypts anything. (D075)
        let built = build(&payload(), Generation::Current).expect("builds");
        let parsed = Container::parse(&built).expect("parses");
        for entry in parsed.entries() {
            if !entry.carries_segment_data() {
                continue;
            }
            assert_eq!(
                entry.memsz, entry.filesz,
                "an uncompressed entry's two sizes are the same number"
            );
            assert_ne!(
                entry.memsz, 0,
                "and a segment with data is never zero blocks"
            );
        }
    }

    #[test]
    fn the_segment_bytes_survive_the_wrapping() {
        let source = payload();
        let built = build(&source, Generation::Current).expect("builds");
        let parsed = Container::parse(&built).expect("parses");
        let data = parsed
            .entries()
            .iter()
            .find(|e| e.carries_segment_data())
            .expect("a data entry");
        let at = usize::try_from(data.offset).expect("offset");
        let len = usize::try_from(data.filesz).expect("length");
        let wrapped = built.get(at..at + len).expect("segment is present");
        let original: Vec<u8> = (0..=255_u8).cycle().take(300).collect();
        assert_eq!(wrapped, original.as_slice(), "the payload was altered");
    }

    #[test]
    fn a_plain_executable_is_reported_as_such_rather_than_as_a_bad_container() {
        let err = Container::parse(&payload()).expect_err("a plain ELF is not a container");
        assert_eq!(err, ContainerError::NotAContainer(selfish_elf::MAGIC));
    }

    #[test]
    fn the_two_generations_produce_different_files() {
        let current = build(&payload(), Generation::Current).expect("builds");
        let previous = build(&payload(), Generation::Previous).expect("builds");
        assert_ne!(
            current.get(..4),
            previous.get(..4),
            "the magic must differ, or one of them is built for the wrong console"
        );
        assert_eq!(
            current.len(),
            previous.len(),
            "everything except the magic is identical between generations"
        );
    }

    #[test]
    fn an_executable_with_no_qualifying_segment_is_refused() {
        let mut bytes = payload();
        // Turn the only PT_LOAD into something that never becomes an entry.
        bytes[64..68].copy_from_slice(&segment::INTERP.to_le_bytes());
        assert_eq!(
            build(&bytes, Generation::Current).expect_err("nothing to describe"),
            ContainerError::NoSegments
        );
    }

    #[test]
    fn a_container_this_crate_builds_confirms_every_fixed_row_of_the_table() {
        // The audit's floor: a file built *from* the table must agree with the table on every
        // fixed row. If it does not, the writer and the row reader disagree about the format,
        // which is the bug the whole `data/` discipline exists to catch. This is the same round
        // trip as principle 4, reached from the reading side.
        for generation in [Generation::Current, Generation::Previous] {
            let built = build(&payload(), generation).expect("builds");
            let result = audit(&built).expect("audits");
            assert_eq!(result.generation, generation);
            let differ = result.differing();
            assert!(
                differ.is_empty(),
                "{generation:?}: rows this crate wrote and then failed to confirm: {differ:?}",
            );
            assert!(result.confirmed() > 0, "the header pins some fixed rows");
        }
    }

    #[test]
    fn the_magic_is_not_counted_as_a_row_the_file_got_wrong() {
        // The magic differs by generation on purpose. An audit that counted it as a mismatch
        // would report the generation split itself as an error against every file of the other
        // generation. It is read to identify the generation and then left out of the tally.
        let built = build(&payload(), Generation::Current).expect("builds");
        let result = audit(&built).expect("audits");
        assert!(
            result.header.iter().all(|row| row.field != "magic"),
            "the magic must not appear as a checked row",
        );
    }

    #[test]
    fn a_changed_header_byte_is_reported_as_a_difference() {
        // The point of the tool on a real file: a byte that does not match the table is found
        // and named. Here the `flags` field at 0x1A is corrupted and the audit must catch it.
        let mut built = build(&payload(), Generation::Current).expect("builds");
        built[0x1A] ^= 0xFF;
        let result = audit(&built).expect("audits");
        assert!(
            result.differing().iter().any(|row| row.field == "flags"),
            "a corrupted flags field must show up as differing: {:?}",
            result.differing(),
        );
    }
}
