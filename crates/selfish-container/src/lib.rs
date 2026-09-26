//! The signed-executable container, read and written.
//!
//! What the platform wraps an executable in. The containers built here declare themselves
//! fake in the format's own field, with every digest and the signature area zero (D047).
//!
//! ```text
//! [header 32][entry 32 x N][ELF header + program headers][pad][ex_info 64][npdrm 48]  <- header_size
//! [meta_block 80 x N][meta_footer 80][signature 256]                                  <- meta_size
//! [payloads, each aligned]
//! ```
//!
//! Each segment has two entries: a digest entry with one digest per block, then the segment
//! itself. Every constant comes from `data/self-format.tsv` through [`table`].

#![forbid(unsafe_code)]

pub mod sdk;
pub mod table;

use core::fmt;

pub use sdk::{SdkDictionary, SdkEntry, TargetSdk, patch_elf_procparam};
use selfish_abi::Generation;
use selfish_bytes::read_le;
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

/// Program header types that become container entries, read from the table.
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
    /// The bit a loader searches on. A container where no entry carries it parses and then
    /// maps nothing.
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

        let field = |at| read_le::<u16>(head, at).ok_or(ContainerError::TooShort);
        let header_size = u64::from(field(0x0C)?);
        let meta_size = u64::from(field(0x0E)?);
        let file_size = read_le::<u64>(head, 0x10).ok_or(ContainerError::TooShort)?;
        let count = field(0x18)?;

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
            let field = |at| read_le::<u64>(raw, at).ok_or(ContainerError::TooShort);
            entries.push(Entry {
                props: field(0)?,
                offset: field(8)?,
                filesz: field(16)?,
                memsz: field(24)?,
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

    /// Which hardware generation this container is for.
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
    /// Immediately after the entry table, so it depends on the entry count.
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
    /// If the derived offset does not hold an executable.
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
    /// The inverse of [`build`]. The headers sit after the entry table and each segment's
    /// contents wherever its entry says; each is written back to the file offset its program
    /// header names. Digest entries hold no contents and are skipped.
    ///
    /// # Errors
    ///
    /// If the inner headers are absent or malformed, or an entry points outside the file.
    pub fn to_elf(&self) -> Result<Vec<u8>, ContainerError> {
        let inner = self.inner_elf_header()?;
        let elf = Elf::parse(inner)?;
        let span = usize::try_from(elf.header_span())
            .map_err(|_| ContainerError::Arithmetic("header span"))?;

        // Sized by the furthest any program header reaches; the container's metadata has no
        // place in the executable.
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
                // A segment the executable does not have is skipped; the rest is recoverable.
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
/// So 16KiB is written as 2. The field is four bits wide.
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

/// What a container says about its own kind, read where the table pins `ex_info.ptype`.
///
/// A container written from this table matches it, so an audit of a fake container is a round
/// trip and says nothing about vendor material. The value is reported, not tested: an
/// unrecognised `ptype` is returned with no name rather than treated as not found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declared {
    /// The value at the offset the table pins for `ex_info.ptype`.
    Ptype {
        /// The raw value, whatever it turns out to be.
        value: u64,
        /// The name the `ptype` group gives it, if it names it at all.
        known: Option<String>,
    },
    /// The tail is not where the table says it is, or the file stops before it.
    ///
    /// Not an error, and not a judgement about the format: `header_size - 0x70` is an
    /// Orbis-derived layout, and a container whose tail sits elsewhere reads as this.
    Unreachable,
}

impl Declared {
    /// Whether a matching audit would only be a round trip.
    ///
    /// True when the container declares itself fake, where agreement with this table is
    /// guaranteed in advance. `false` for a value this crate cannot place; [`Self::caveat`]
    /// qualifies that case.
    #[must_use]
    pub fn is_round_trip(&self) -> bool {
        matches!(self, Self::Ptype { known, .. } if known.as_deref() == Some("fake"))
    }

    /// A line to print beside a verdict, or `None` when the verdict stands unqualified.
    #[must_use]
    pub fn caveat(&self) -> Option<&'static str> {
        match self {
            Self::Ptype { known, .. } if known.as_deref() == Some("fake") => Some(
                "this container declares itself FAKE - it agrees with the table because it was \
                 written from one, so a match is a round trip and says nothing about vendor \
                 material",
            ),
            Self::Ptype { known: None, .. } => Some(
                "the value where `ex_info.ptype` should be is not one this table names - either \
                 this is a kind of container the table does not record, or the tail is not at \
                 `header_size - 0x70`",
            ),
            Self::Unreachable => Some(
                "`ex_info` could not be reached, so what kind of container this is was never \
                 established - a match cannot be told apart from a round trip",
            ),
            Self::Ptype { .. } => None,
        }
    }
}

/// Where `ex_info` starts, by the layout this table pins.
///
/// `[self_header 32][entry 32 x N][ELF ehdr + phdrs][pad to 16][ex_info 64][npdrm 48]` ends at
/// `header_size`, so the two tail blocks are the last `0x70` bytes of the header.
fn ex_info_at(bytes: &[u8]) -> Option<usize> {
    let header_size = read_at(bytes, 12, 2)?;
    let at = usize::try_from(header_size).ok()?.checked_sub(0x70)?;
    (at.checked_add(0x70)? <= bytes.len()).then_some(at)
}

/// Read the `ptype` where the table pins it, without deciding whether the answer is plausible.
fn declared_kind(bytes: &[u8]) -> Declared {
    let Some(value) = ex_info_at(bytes)
        .and_then(|ex| ex.checked_add(8))
        .and_then(|at| read_at(bytes, at, 8))
    else {
        return Declared::Unreachable;
    };
    let known = table::group("ptype")
        .into_iter()
        .find(|(_, candidate)| *candidate == value)
        .map(|(name, _)| name);
    Declared::Ptype { value, known }
}

/// Check the `ex_info` rows the table pins, rebased onto the file.
///
/// Empty when the tail is not reachable, the same condition [`Declared::Unreachable`] reports.
/// The rows are the values a fake container carries, so a vendor container may differ here.
fn tail_rows(bytes: &[u8]) -> Vec<RowVerdict> {
    let Some(base) = ex_info_at(bytes) else {
        return Vec::new();
    };
    table::fixed_fields("ex_info")
        .into_iter()
        .filter_map(|row| {
            let at = base.checked_add(row.offset)?;
            let found = read_at(bytes, at, row.size);
            Some(RowVerdict {
                matched: found == Some(row.value),
                field: row.field,
                offset: at,
                expected: row.value,
                found,
                note: row.note,
            })
        })
        .collect()
}

/// The result of checking a real container against the table.
///
/// `data/self-format.tsv` is derived from cited Orbis-generation sources; a real file confirms
/// or contradicts each row. A contradicted row is reported with the real value beside the
/// expected one and is not interpreted: what the field means needs a citable source.
#[derive(Debug, Clone)]
pub struct Audit {
    /// The generation the magic identifies.
    pub generation: Generation,
    /// One verdict per fixed row in the header.
    pub header: Vec<RowVerdict>,
    /// One verdict per fixed row of `ex_info`, rebased onto the file.
    ///
    /// Empty when the tail is not reachable. These are the values a fake container carries,
    /// so a vendor container may differ here.
    pub tail: Vec<RowVerdict>,
    /// What the container says its own kind is. See [`Declared`].
    pub declared: Declared,
}

impl Audit {
    /// How many rows the file confirmed.
    #[must_use]
    pub fn confirmed(&self) -> usize {
        self.header.iter().filter(|row| row.matched).count()
    }

    /// The `ex_info` rows the file contradicted.
    ///
    /// Separate from [`Self::differing`]: a header row differing is a claim about the format,
    /// while a tail row differing usually means the container is not fake.
    #[must_use]
    pub fn tail_differing(&self) -> Vec<&RowVerdict> {
        self.tail.iter().filter(|row| !row.matched).collect()
    }

    /// The header rows the file contradicted.
    #[must_use]
    pub fn differing(&self) -> Vec<&RowVerdict> {
        self.header.iter().filter(|row| !row.matched).collect()
    }
}

/// Check a real container against the format table.
///
/// The magic identifies the generation and is not checked as a row, since it differs by
/// generation by design. Every other fixed header row is checked.
///
/// # Errors
///
/// If the bytes are too short to hold a header, or the magic is not a container's.
pub fn audit(bytes: &[u8]) -> Result<Audit, ContainerError> {
    let container = Container::parse(bytes)?;
    let generation = container.generation();

    let mut header = Vec::new();
    for row in table::fixed_fields("self_header") {
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
    Ok(Audit {
        generation,
        header,
        tail: tail_rows(bytes),
        declared: declared_kind(bytes),
    })
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

/// The privilege tier for an executable container.
///
/// Dictates the `paid` (Program Authentication ID) stamped into `self_ex_info`, through
/// [`Privilege::paid`]:
/// - `App`: the format's default, `0x3100000000000002`, from `data/self-format.tsv`.
/// - `Sysmodule`: the same as `App`, so the container is byte-identical.
/// - `System`: `0x3800000000000001`.
/// - `Root`: `0x8000000000000001`.
///
/// What each tier is granted on hardware is not measured here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Privilege {
    /// Standard title sandbox (Tier 1).
    #[default]
    App,
    /// Title sandbox with dynamic sysmodule access (Tier 2).
    Sysmodule,
    /// Extended system application (Tier 3, `/system_ex` access).
    System,
    /// Root / kernel superuser access (Tier 4, `/system/priv` access).
    Root,
}

impl Privilege {
    /// Compute the Program Authentication ID (`paid`) for this privilege tier.
    #[must_use]
    pub const fn paid(self, default_paid: u64) -> u64 {
        match self {
            Self::App | Self::Sysmodule => default_paid,
            Self::System => 0x3800_0000_0000_0001,
            Self::Root => 0x8000_0000_0000_0001,
        }
    }
}

impl core::str::FromStr for Privilege {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "app" | "game" | "unprivileged" => Ok(Self::App),
            "sysmodule" => Ok(Self::Sysmodule),
            "system" | "sys" => Ok(Self::System),
            "root" | "admin" | "kernel" => Ok(Self::Root),
            _ => Err("unknown privilege tier: expected app, sysmodule, system, or root"),
        }
    }
}

/// Wrap an executable in a fake container for a given generation.
///
/// # Errors
///
/// If the payload is not a usable executable, no segment qualifies as an entry, a size
/// computation overflows, or the format table is missing a constant.
pub fn build(payload: &[u8], generation: Generation) -> Result<Vec<u8>, ContainerError> {
    build_with_options(payload, generation, Privilege::App, None)
}

/// Wrap an executable in a fake container with explicit privilege tier.
///
/// # Errors
///
/// If the payload is not a usable executable, no segment qualifies as an entry, a size
/// computation overflows, or the format table is missing a constant.
pub fn build_with_privilege(
    payload: &[u8],
    generation: Generation,
    privilege: Privilege,
) -> Result<Vec<u8>, ContainerError> {
    build_with_options(payload, generation, privilege, None)
}

/// Wrap an executable in a fake container with explicit privilege tier and target SDK version.
///
/// When an SDK target is provided, any `PT_SCE_PROCPARAM` segment in the ELF is stamped
/// with the validated Orbis and Prospero (PPR) SDK versions.
///
/// # Errors
///
/// If the payload is not a usable executable, no segment qualifies as an entry, a size
/// computation overflows, or the format table is missing a constant.
pub fn build_with_options(
    payload: &[u8],
    generation: Generation,
    privilege: Privilege,
    sdk: Option<TargetSdk>,
) -> Result<Vec<u8>, ContainerError> {
    let mut payload_buf;
    let payload_ref = if let Some(target_sdk) = sdk {
        payload_buf = payload.to_vec();
        patch_elf_procparam(&mut payload_buf, target_sdk);
        &payload_buf[..]
    } else {
        payload
    };

    let mut constants = Constants::load()?;
    constants.paid = privilege.paid(constants.paid);
    let elf = Elf::parse(payload_ref)?;
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
    out.raw(payload_ref.get(..span).ok_or(ContainerError::Arithmetic(
        "payload shorter than its headers",
    ))?);
    out.pad_to(prefix_end(&constants, entry_count, ehdr_span)?);
    write_ex_info(&mut out, &constants);
    write_npdrm(&mut out, &constants);
    out.pad_to(usize::try_from(header_size).map_err(|_| ContainerError::Arithmetic("header"))?);
    write_metadata(&mut out, &constants, entry_count)?;
    write_payloads(&mut out, payload_ref, &chosen, &entries)?;
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
/// Both the entry table and the payload writer read this plan.
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

        // The data entry is what a loader searches for; its index is the program header's,
        // not this entry's position.
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
            // The data's uncompressed size, not the segment's `p_memsz`; nothing is compressed,
            // so it is `filesz`. The loader divides it by the block size to count blocks, and
            // `PT_SCE_DYNLIBDATA` has `p_memsz` zero because it is never mapped.
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

/// `ptype` declares the container fake.
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

/// One block per entry, a footer, and the signature area, all zero but one constant.
///
/// Zero digests and a zero signature make no claim about content, matching `ptype`.
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
    /// Carries what was found; the usual cause is a plain executable.
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
    use super::{
        Constants, Container, ContainerError, Declared, Entry, audit, build, entry_segment_types,
    };
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

    /// Every constant the builder needs is present in the format table.
    #[test]
    fn every_constant_the_builder_needs_is_in_the_table() {
        Constants::load().expect("the format table is missing a constant this code needs");
    }

    /// The table marks exactly the four entry-producing segment types.
    #[test]
    fn four_segment_types_become_entries() {
        let types = entry_segment_types();
        assert_eq!(types.len(), 4, "LOAD, RELRO, DYNLIBDATA, COMMENT");
        assert!(types.contains(&segment::LOAD));
        assert!(types.contains(&segment::SCE_DYNLIBDATA));
    }

    /// A built container parses back with its generation, entries, size and inner ELF.
    #[test]
    fn a_container_round_trips_through_its_own_parser() {
        for generation in [Generation::Prospero, Generation::Orbis] {
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

    /// A data entry carries the segment-data bit a loader searches for.
    #[test]
    fn the_data_entry_carries_the_bit_a_loader_searches_for() {
        let built = build(&payload(), Generation::Prospero).expect("builds");
        let parsed = Container::parse(&built).expect("parses");
        assert!(
            parsed.entries().iter().any(Entry::carries_segment_data),
            "no entry carries the segment-data bit; a loader would find nothing to map"
        );
    }

    /// A data entry's segment index is its program header's index.
    #[test]
    fn the_data_entry_points_at_its_program_header_not_at_itself() {
        let built = build(&payload(), Generation::Prospero).expect("builds");
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

    /// A data entry's `memsz` is its data size, never the segment's `p_memsz`.
    #[test]
    fn a_data_entry_sizes_itself_by_its_data_and_never_by_p_memsz() {
        let built = build(&payload(), Generation::Prospero).expect("builds");
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

    /// Segment bytes are stored unchanged at the data entry's offset.
    #[test]
    fn the_segment_bytes_survive_the_wrapping() {
        let source = payload();
        let built = build(&source, Generation::Prospero).expect("builds");
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

    /// A plain executable is reported as not a container, with its magic.
    #[test]
    fn a_plain_executable_is_reported_as_such_rather_than_as_a_bad_container() {
        let err = Container::parse(&payload()).expect_err("a plain ELF is not a container");
        assert_eq!(err, ContainerError::NotAContainer(selfish_elf::MAGIC));
    }

    /// The two generations differ only in the magic.
    #[test]
    fn the_two_generations_produce_different_files() {
        let prospero = build(&payload(), Generation::Prospero).expect("builds");
        let orbis = build(&payload(), Generation::Orbis).expect("builds");
        assert_ne!(
            prospero.get(..4),
            orbis.get(..4),
            "the magic must differ, or one of them is built for the wrong generation"
        );
        assert_eq!(
            prospero.len(),
            orbis.len(),
            "everything except the magic is identical between generations"
        );
    }

    /// An executable with no entry-producing segment is refused.
    #[test]
    fn an_executable_with_no_qualifying_segment_is_refused() {
        let mut bytes = payload();
        // Turn the only PT_LOAD into something that never becomes an entry.
        bytes[64..68].copy_from_slice(&segment::INTERP.to_le_bytes());
        assert_eq!(
            build(&bytes, Generation::Prospero).expect_err("nothing to describe"),
            ContainerError::NoSegments
        );
    }

    /// A container built from the table confirms every fixed header row of the table.
    #[test]
    fn a_container_this_crate_builds_confirms_every_fixed_row_of_the_table() {
        for generation in [Generation::Prospero, Generation::Orbis] {
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

    /// The audit leaves the magic out of the checked rows.
    #[test]
    fn the_magic_is_not_counted_as_a_row_the_file_got_wrong() {
        let built = build(&payload(), Generation::Prospero).expect("builds");
        let result = audit(&built).expect("audits");
        assert!(
            result.header.iter().all(|row| row.field != "magic"),
            "the magic must not appear as a checked row",
        );
    }

    /// A corrupted header field is named as differing.
    #[test]
    fn a_changed_header_byte_is_reported_as_a_difference() {
        let mut built = build(&payload(), Generation::Prospero).expect("builds");
        built[0x1A] ^= 0xFF;
        let result = audit(&built).expect("audits");
        assert!(
            result.differing().iter().any(|row| row.field == "flags"),
            "a corrupted flags field must show up as differing: {:?}",
            result.differing(),
        );
    }

    /// The audit of a built container reports it as fake and caveats the match as a round trip.
    #[test]
    fn a_container_this_crate_builds_says_it_is_fake_and_the_audit_says_so_too() {
        for generation in [Generation::Prospero, Generation::Orbis] {
            let built = build(&payload(), generation).expect("builds");
            let result = audit(&built).expect("audits");

            assert!(
                result.differing().is_empty(),
                "{generation:?}: precondition - this is the round trip",
            );
            assert!(
                result.declared.is_round_trip(),
                "{generation:?}: a container this crate built must declare itself fake, so the \
                 match above cannot be mistaken for evidence: {:?}",
                result.declared,
            );
            assert!(
                result
                    .declared
                    .caveat()
                    .is_some_and(|line| line.contains("round trip")),
                "{generation:?}: and the caveat has to say it in words",
            );
        }
    }

    /// An unrecognised `ptype` is reported with its raw value, not treated as not found.
    #[test]
    fn a_ptype_this_table_does_not_name_is_reported_rather_than_refused() {
        let mut built = build(&payload(), Generation::Prospero).expect("builds");
        let header_size = usize::from(u16::from_le_bytes([built[12], built[13]]));
        let at = header_size - 0x70 + 8;
        built[at..at + 8].copy_from_slice(&0x1234_u64.to_le_bytes());

        let result = audit(&built).expect("audits");
        match &result.declared {
            Declared::Ptype { value, known } => {
                assert_eq!(*value, 0x1234, "the raw value comes back whatever it is");
                assert!(known.is_none(), "and this table does not name it");
            }
            other @ Declared::Unreachable => {
                panic!("expected the value to be reported, got {other:?}")
            }
        }
        assert!(
            !result.declared.is_round_trip(),
            "unknown is not known-to-be-fake"
        );
        assert!(
            result.declared.caveat().is_some(),
            "and it is still caveated"
        );
    }

    /// A `header_size` past the end of the file makes the tail unreachable.
    #[test]
    fn a_tail_that_is_named_but_not_there_is_unreachable_rather_than_a_guess() {
        // Truncating the file instead fails earlier, in `parse`.
        let mut built = build(&payload(), Generation::Prospero).expect("builds");
        built[12..14].copy_from_slice(&u16::MAX.to_le_bytes());
        let result = audit(&built).expect("audits");
        assert_eq!(result.declared, Declared::Unreachable);
        assert!(!result.declared.is_round_trip());
        assert!(result.declared.caveat().is_some());
    }

    /// A container built from the table confirms every `ex_info` tail row.
    #[test]
    fn a_container_this_crate_builds_confirms_the_tail_rows_as_well() {
        for generation in [Generation::Prospero, Generation::Orbis] {
            let built = build(&payload(), generation).expect("builds");
            let result = audit(&built).expect("audits");

            assert_eq!(
                result.tail.len(),
                4,
                "{generation:?}: paid, ptype, app_version, fw_version"
            );
            assert!(
                result.tail_differing().is_empty(),
                "{generation:?}: tail rows this crate wrote and then failed to confirm: {:?}",
                result.tail_differing(),
            );
        }
    }

    /// A differing tail row reports the value the file holds beside the expected one.
    #[test]
    fn a_tail_row_that_differs_reports_the_value_the_file_holds() {
        let mut built = build(&payload(), Generation::Prospero).expect("builds");
        let base = usize::from(u16::from_le_bytes([built[12], built[13]])) - 0x70;
        built[base..base + 8].copy_from_slice(&0x1b_ac98_u64.to_le_bytes());

        let result = audit(&built).expect("audits");
        let differing = result.tail_differing();
        assert_eq!(differing.len(), 1, "one row changed, one row differs");
        assert_eq!(differing[0].field, "paid");
        assert_eq!(differing[0].found, Some(0x1b_ac98));
        assert_eq!(differing[0].expected, 0x3100_0000_0000_0002);
    }

    /// An unreachable tail yields no tail rows, agreeing with `declared`.
    #[test]
    fn an_unreachable_tail_yields_no_rows_rather_than_rows_read_from_nowhere() {
        let mut built = build(&payload(), Generation::Prospero).expect("builds");
        built[12..14].copy_from_slice(&u16::MAX.to_le_bytes());

        let result = audit(&built).expect("audits");
        assert!(result.tail.is_empty());
        assert_eq!(result.declared, Declared::Unreachable);
    }

    /// `to_elf` splices each segment's bytes back into the executable it returns.
    #[test]
    fn the_elf_a_container_gives_back_carries_its_segment_payloads() {
        for generation in [Generation::Prospero, Generation::Orbis] {
            let original = payload();
            let built = build(&original, generation).expect("builds");
            let inner = Container::parse(&built)
                .expect("parses")
                .to_elf()
                .expect("splices");

            let elf = selfish_elf::Elf::parse(&inner).expect("the spliced ELF parses");
            let phdr = elf.program_headers().first().expect("one segment");
            let bytes = elf
                .segment_bytes(phdr)
                .expect("and its contents are present, which is the whole point");

            // 64 + 56: the fixture puts its payload immediately after ehdr and one phdr.
            assert_eq!(
                bytes,
                &original[120..],
                "{generation:?}: the payload came back byte for byte",
            );
        }
    }
}
