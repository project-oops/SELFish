//! The AGC shader container that `sceAgcCreateShader` is handed.
//!
//! A fixed header, followed by the sub-tables its pointer fields reach. The layout comes from
//! `data/agc-shader-format.tsv`, which five open-source emulators agree on and one
//! pins with a `static_assert`; obSCEne's hardware sweep is the oracle a real container passed.
//! (selfish D103)
//!
//! # Format here, contents from the caller
//!
//! This crate owns the container *format* - the header, and where the sub-tables sit and how the
//! pointer fields reach them. It does not own the sub-table *contents*: which registers a shader
//! programs and its resource usage are read out of the shader's own RDNA2 bytecode by whatever
//! produced it, and are handed in. That line is the admission test - laying out a container is
//! knowledge about a format; parsing bytecode to find a shader's registers is what a compiler
//! knows. (D103)
//!
//! # The pointers are self-relative
//!
//! A pointer field does not hold an absolute address in a built container. It holds an offset
//! relative to the field's own location, and `sceAgcCreateShader` rewrites it in place to
//! `field_address + offset` (craziiEmu `RelocatePointerField`). So [`Container::build`] sets each
//! pointer field to `sub_table_offset - field_offset`, and [`relocate`] does what a console does,
//! which is how the round-trip test reads the sub-tables back.

/// The format table, with its provenance header. Code reads this rather than carrying offsets.
const FORMAT: &str = include_str!("../../../data/agc-shader-format.tsv");

/// A single GPU register entry: an address and the value written to it.
///
/// The program-address registers a console patches from the code pointer; the rest are the
/// shader's own. This crate places them and does not interpret them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderRegister {
    /// The GPU register address.
    pub offset: u32,
    /// The value written to it.
    pub value: u32,
}

/// One shader stage's container, ready to lay out.
///
/// The scalar fields are the shader's own metadata, supplied rather than derived. The register
/// arrays are placed as sub-tables; empty ones leave their pointer field zero.
#[derive(Debug, Clone, Default)]
pub struct Container {
    /// The stage `type` byte. See [`STAGE_COMPUTE`], [`STAGE_PIXEL`], [`STAGE_VERTEX`], and the
    /// `type` row of `data/agc-shader-format.tsv` for the values and what a source establishes.
    pub stage: u8,
    /// The ISA/target the bytecode is for.
    pub target: u32,
    /// The bytecode's size in bytes.
    pub shader_size: u32,
    /// Embedded constant buffer size, in dqwords.
    pub embedded_constant_buffer_size_dqw: u32,
    /// Scratch dwords per thread.
    pub scratch_size_dw_per_thread: u16,
    /// SH registers - for a compute shader, the program-address pair and its resource registers.
    pub sh_registers: Vec<ShaderRegister>,
    /// Context registers.
    pub cx_registers: Vec<ShaderRegister>,
}

impl Container {
    /// A compute shader container from its bytecode size and SH registers.
    ///
    /// The registers are the shader's, read from its bytecode by the caller; the program-address
    /// pair (compute program lo/hi) a console patches from the code pointer at create time.
    #[must_use]
    pub fn compute(shader_size: u32, target: u32, sh_registers: Vec<ShaderRegister>) -> Self {
        Self::for_stage(STAGE_COMPUTE, shader_size, target, sh_registers)
    }

    /// A pixel shader container. Stage [`STAGE_PIXEL`].
    #[must_use]
    pub fn pixel(shader_size: u32, target: u32, sh_registers: Vec<ShaderRegister>) -> Self {
        Self::for_stage(STAGE_PIXEL, shader_size, target, sh_registers)
    }

    /// A vertex shader container. Stage [`STAGE_VERTEX`].
    #[must_use]
    pub fn vertex(shader_size: u32, target: u32, sh_registers: Vec<ShaderRegister>) -> Self {
        Self::for_stage(STAGE_VERTEX, shader_size, target, sh_registers)
    }

    /// A container for a given stage `type` value.
    ///
    /// The named constructors ([`compute`](Self::compute), [`pixel`](Self::pixel),
    /// [`vertex`](Self::vertex)) cover the stage values a citable source establishes. This is the
    /// general primitive for any other stage: a caller that has confirmed a `type` value - from
    /// hardware, say - passes it directly, and the container is laid out around it. The crate does
    /// not assert what an arbitrary `type` means; only the stage values with a named constant are
    /// ones it stands behind. The registers are the shader's, supplied by the caller.
    #[must_use]
    pub fn for_stage(
        stage: u8,
        shader_size: u32,
        target: u32,
        sh_registers: Vec<ShaderRegister>,
    ) -> Self {
        Self {
            stage,
            target,
            shader_size,
            sh_registers,
            ..Self::default()
        }
    }

    /// Lay the container out: the header, then the register sub-tables, with each pointer field
    /// set to the self-relative offset a console relocates.
    ///
    /// # Panics
    ///
    /// If `data/agc-shader-format.tsv` has lost a row this needs. The table is compiled in, so a
    /// missing row is a table that changed shape while this code did not - not something to paper
    /// over with a plausible offset in a structure a console reads.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        let header_size = size_of_group("header");
        let mut out = vec![0_u8; header_size];

        // Fixed identity, then the caller's scalars.
        write_bytes(&mut out, offset_of("header", "file_header"), &magic());
        put_u32(&mut out, offset_of("header", "version"), version());
        put_u32(
            &mut out,
            offset_of("header", "header_size"),
            as_u32(header_size),
        );
        put_u32(
            &mut out,
            offset_of("header", "shader_size"),
            self.shader_size,
        );
        put_u32(
            &mut out,
            offset_of("header", "embedded_constant_buffer_size_dqw"),
            self.embedded_constant_buffer_size_dqw,
        );
        put_u32(&mut out, offset_of("header", "target"), self.target);
        put_u16(
            &mut out,
            offset_of("header", "scratch_size_dw_per_thread"),
            self.scratch_size_dw_per_thread,
        );
        put_u8(&mut out, offset_of("header", "type"), self.stage);
        put_u8(
            &mut out,
            offset_of("header", "num_cx_registers"),
            as_u8(self.cx_registers.len()),
        );
        put_u8(
            &mut out,
            offset_of("header", "num_sh_registers"),
            as_u8(self.sh_registers.len()),
        );

        // Each register array becomes a sub-table after the header, reached by a self-relative
        // pointer. An empty array leaves its field zero, which is a null pointer to a console.
        append_registers(
            &mut out,
            offset_of("header", "cx_registers"),
            &self.cx_registers,
        );
        append_registers(
            &mut out,
            offset_of("header", "sh_registers"),
            &self.sh_registers,
        );

        out
    }
}

/// Place a register array at the end of `out` and point `field` at it, self-relative.
///
/// Does nothing for an empty array: the pointer field stays zero, which is the null a console
/// reads as "no sub-table".
fn append_registers(out: &mut Vec<u8>, field: usize, registers: &[ShaderRegister]) {
    if registers.is_empty() {
        return;
    }
    let at = out.len();
    for register in registers {
        put_u32_push(out, register.offset);
        put_u32_push(out, register.value);
    }
    // field_address + offset = sub_table_address, so offset = at - field.
    let relative = as_u64(at).wrapping_sub(as_u64(field));
    put_u64(out, field, relative);
}

/// Resolve one pointer field the way `sceAgcCreateShader` does: `field += field_address`.
///
/// Returns the absolute offset of the sub-table within `container`, or `None` for a null field.
/// This is what the round-trip test uses to read a built container back.
#[must_use]
pub fn relocate(container: &[u8], field: usize) -> Option<usize> {
    let relative = read_u64(container, field)?;
    if relative == 0 {
        return None;
    }
    usize::try_from(as_u64(field).wrapping_add(relative)).ok()
}

/// Read a register array a pointer field reaches, given the field offset and the count.
#[must_use]
pub fn registers_at(container: &[u8], field: usize, count: usize) -> Option<Vec<ShaderRegister>> {
    let base = relocate(container, field)?;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let at = base.checked_add(index.checked_mul(8)?)?;
        out.push(ShaderRegister {
            offset: read_u32(container, at)?,
            value: read_u32(container, at.checked_add(4)?)?,
        });
    }
    Some(out)
}

/// The compute stage `type`. craziiEmu maps it to the compute program registers; the 9f4c
/// hardware probe's baseline is a compute container the console accepts.
pub const STAGE_COMPUTE: u8 = 0;

/// The pixel (fragment) stage `type`. craziiEmu's `1 => SpiShaderPgmLoPs`; obSCEne's
/// `166-agc/primitive-draw` uses it for the fragment stage on hardware.
pub const STAGE_PIXEL: u8 = 1;

/// The vertex stage `type`. craziiEmu's `2 => SpiShaderPgmLoEs` - the "export shader", the
/// hardware stage a vertex shader runs in; obSCEne's `166-agc/primitive-draw` draws with `type=2`
/// as its vertex stage. Named `vertex` for the API stage it serves, `ES` for the hardware one.
///
/// The higher stages obSCEne's request named - geometry, hull - are deliberately **not** given
/// constants here: their values conflict with the only citable header-byte source (craziiEmu has
/// `4=GS`, `7=LS`, no `3`), and the 9f4c probe did not exercise them. A caller that has confirmed
/// such a value builds it with [`Container::for_stage`] rather than a constant this crate cannot
/// yet stand behind. (D103)
pub const STAGE_VERTEX: u8 = 2;

/// The `file_header` magic bytes, from the table.
///
/// # Panics
///
/// If the compiled-in table has lost the row - see [`Container::build`].
#[must_use]
pub fn magic() -> Vec<u8> {
    bytes_of("header", "file_header")
}

/// The `version` value, from the table.
///
/// # Panics
///
/// If the compiled-in table has lost the row - see [`Container::build`].
#[must_use]
pub fn version() -> u32 {
    u32::try_from(number_of("header", "version"))
        .unwrap_or_else(|_| panic!("header/version does not fit in a u32"))
}

/// One field's offset within its group.
///
/// # Panics
///
/// If the row is absent - a compiled-in table that changed shape (see [`Container::build`]).
#[must_use]
pub fn offset_of(group: &str, field: &str) -> usize {
    let row = row_of(group, field)
        .unwrap_or_else(|| panic!("agc-shader-format.tsv has no row for {group}/{field}"));
    parse_number(row.offset)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| panic!("{group}/{field} has no usable offset"))
}

/// A group's `(size)` row, in bytes.
fn size_of_group(group: &str) -> usize {
    let row = row_of(group, "(size)")
        .unwrap_or_else(|| panic!("agc-shader-format.tsv has no {group}/(size) row"));
    parse_number(row.size)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| panic!("{group}/(size) is not a size"))
}

/// One row of the table, as the columns this crate reads.
#[derive(Clone, Copy)]
struct Row<'a> {
    offset: &'a str,
    size: &'a str,
    value: &'a str,
}

fn row_of(group: &str, field: &str) -> Option<Row<'static>> {
    FORMAT
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let columns: Vec<&str> = line.split('\t').collect();
            if columns.first() != Some(&group) || columns.get(1) != Some(&field) {
                return None;
            }
            Some(Row {
                offset: columns.get(2).copied().unwrap_or_default(),
                size: columns.get(3).copied().unwrap_or_default(),
                value: columns.get(5).copied().unwrap_or_default(),
            })
        })
}

fn number_of(group: &str, field: &str) -> u64 {
    let row = row_of(group, field)
        .unwrap_or_else(|| panic!("agc-shader-format.tsv has no row for {group}/{field}"));
    parse_number(row.value).unwrap_or_else(|| panic!("{group}/{field} has no numeric value"))
}

fn bytes_of(group: &str, field: &str) -> Vec<u8> {
    let row = row_of(group, field)
        .unwrap_or_else(|| panic!("agc-shader-format.tsv has no row for {group}/{field}"));
    row.value
        .split_whitespace()
        .map(|byte| {
            u8::from_str_radix(byte, 16)
                .unwrap_or_else(|_| panic!("{group}/{field} is not hex bytes: {:?}", row.value))
        })
        .collect()
}

fn parse_number(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    trimmed.strip_prefix("0x").map_or_else(
        || trimmed.parse::<u64>().ok(),
        |hex| u64::from_str_radix(hex, 16).ok(),
    )
}

fn as_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or_else(|_| panic!("value {value} does not fit in a u32"))
}

fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or_else(|_| panic!("value {value} does not fit in a u64"))
}

fn as_u8(value: usize) -> u8 {
    u8::try_from(value).unwrap_or_else(|_| panic!("count {value} does not fit in a u8"))
}

fn write_bytes(out: &mut [u8], at: usize, bytes: &[u8]) {
    if let Some(slot) = out.get_mut(at..at.saturating_add(bytes.len())) {
        slot.copy_from_slice(bytes);
    }
}

fn put_u8(out: &mut [u8], at: usize, value: u8) {
    if let Some(slot) = out.get_mut(at) {
        *slot = value;
    }
}

fn put_u16(out: &mut [u8], at: usize, value: u16) {
    write_bytes(out, at, &value.to_le_bytes());
}

fn put_u32(out: &mut [u8], at: usize, value: u32) {
    write_bytes(out, at, &value.to_le_bytes());
}

fn put_u64(out: &mut [u8], at: usize, value: u64) {
    write_bytes(out, at, &value.to_le_bytes());
}

fn put_u32_push(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

fn read_u64(bytes: &[u8], at: usize) -> Option<u64> {
    let slice = bytes.get(at..at.checked_add(8)?)?;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{
        Container, STAGE_COMPUTE, STAGE_PIXEL, STAGE_VERTEX, ShaderRegister, magic, offset_of,
        registers_at, relocate, version,
    };

    /// The compute program-address registers, from craziiEmu: lo at 0x20C, hi at 0x20D. Their
    /// values are placeholders a console patches from the code pointer; here they exercise the
    /// register sub-table.
    fn compute_regs() -> Vec<ShaderRegister> {
        vec![
            ShaderRegister {
                offset: 0x20C,
                value: 0,
            },
            ShaderRegister {
                offset: 0x20D,
                value: 0,
            },
        ]
    }

    #[test]
    fn the_magic_and_version_are_what_five_readers_validate() {
        assert_eq!(magic(), vec![0x31, 0x32, 0x33, 0x34], "'1234' = 0x34333231");
        assert_eq!(version(), 0x18);
        let built = Container::compute(256, 0x0E, compute_regs()).build();
        assert_eq!(&built[0x00..0x04], &[0x31, 0x32, 0x33, 0x34]);
        assert_eq!(
            u32::from_le_bytes(built[0x04..0x08].try_into().unwrap()),
            0x18
        );
    }

    #[test]
    fn the_offsets_are_the_pinned_ones() {
        // prosper's static_assert: user_data 0x08, code 0x10, specials 0x28, type 0x5a,
        // num_sh_registers 0x5c.
        assert_eq!(offset_of("header", "user_data"), 0x08);
        assert_eq!(offset_of("header", "code"), 0x10);
        assert_eq!(offset_of("header", "specials"), 0x28);
        assert_eq!(offset_of("header", "type"), 0x5a);
        assert_eq!(offset_of("header", "num_sh_registers"), 0x5c);
        // and the field the disassembly draft got wrong.
        assert_eq!(offset_of("header", "shader_size"), 0x44);
        assert_eq!(offset_of("header", "target"), 0x4c);
    }

    #[test]
    fn the_scalar_metadata_lands_where_the_table_says() {
        let built = Container::compute(0x400, 0x0E, compute_regs()).build();
        let at = |o: usize| u32::from_le_bytes(built[o..o + 4].try_into().unwrap());
        assert_eq!(at(0x44), 0x400, "shader_size");
        assert_eq!(at(0x4c), 0x0E, "target");
        assert_eq!(built[0x5a], STAGE_COMPUTE, "type");
        assert_eq!(built[0x5c], 2, "num_sh_registers");
        assert_eq!(built[0x5b], 0, "num_cx_registers");
    }

    #[test]
    fn pixel_and_vertex_differ_from_compute_only_in_the_type_byte() {
        // The container format is stage-agnostic - only `type` at 0x5a changes. So a pixel or
        // vertex container is a compute one with one byte different, and nothing else.
        let regs = compute_regs();
        let compute = Container::compute(256, 0x0E, regs.clone()).build();
        let pixel = Container::pixel(256, 0x0E, regs.clone()).build();
        let vertex = Container::vertex(256, 0x0E, regs).build();

        assert_eq!(pixel[0x5a], STAGE_PIXEL, "pixel type = 1");
        assert_eq!(vertex[0x5a], STAGE_VERTEX, "vertex type = 2");

        let differs = |a: &[u8], b: &[u8]| -> Vec<usize> {
            (0..a.len().min(b.len()))
                .filter(|i| a[*i] != b[*i])
                .collect()
        };
        assert_eq!(
            differs(&compute, &pixel),
            vec![0x5a],
            "pixel vs compute: only type"
        );
        assert_eq!(
            differs(&compute, &vertex),
            vec![0x5a],
            "vertex vs compute: only type"
        );
    }

    #[test]
    fn for_stage_writes_any_type_value_it_is_given() {
        // The general primitive: a caller with a confirmed stage value builds it directly, with
        // no constant this crate has to stand behind. `4` is craziiEmu's GS, used here only to
        // show the byte is written through unchanged.
        let built = Container::for_stage(4, 256, 0x0E, compute_regs()).build();
        assert_eq!(built[0x5a], 4, "for_stage writes the type it is handed");
    }

    #[test]
    fn a_self_relative_pointer_relocates_to_its_sub_table_and_reads_back() {
        // The whole round trip: build sets sh_registers to a self-relative offset; `relocate`
        // does what a console does; the registers read back byte for byte. (principle 4)
        let regs = compute_regs();
        let built = Container::compute(256, 0x0E, regs.clone()).build();

        let field = offset_of("header", "sh_registers");
        let base = relocate(&built, field).expect("sh_registers relocates to a sub-table");
        assert!(
            base >= 0x60,
            "the sub-table is past the header, at {base:#x}"
        );

        let read = registers_at(&built, field, regs.len()).expect("registers read back");
        assert_eq!(
            read, regs,
            "the registers survive a build-and-relocate round trip"
        );
    }

    #[test]
    fn an_empty_register_array_leaves_a_null_pointer() {
        // cx_registers is empty for this compute shader, so its field must be zero - the null a
        // console reads as "no sub-table", not a self-relative offset to nothing.
        let built = Container::compute(256, 0x0E, compute_regs()).build();
        let cx = offset_of("header", "cx_registers");
        assert_eq!(&built[cx..cx + 8], &[0_u8; 8], "cx_registers is null");
        assert!(relocate(&built, cx).is_none());
    }

    #[test]
    fn the_header_region_is_clear_of_the_sub_tables() {
        // header_size is the header region, and the first sub-table starts at or after it - so a
        // pointer never lands inside the header it points out of.
        let built = Container::compute(256, 0x0E, compute_regs()).build();
        let header_size = u32::from_le_bytes(built[0x40..0x44].try_into().unwrap()) as usize;
        let base = relocate(&built, offset_of("header", "sh_registers")).unwrap();
        assert!(
            base >= header_size,
            "sub-table {base:#x} is at/after header_size {header_size:#x}"
        );
    }
}
