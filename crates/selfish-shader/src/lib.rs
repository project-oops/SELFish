//! The AGC shader container that `sceAgcCreateShader` is handed.
//!
//! A fixed header, followed by the sub-tables its pointer fields reach. The layout comes from
//! `data/agc-shader-format.tsv`, which five open-source emulators agree on and one pins with a
//! `static_assert`.
//!
//! This crate owns the header and sub-table placement; register contents are read from the
//! shader's bytecode by whatever produced it and handed in (D103).
//!
//! A pointer field holds an offset relative to its own location, which `sceAgcCreateShader`
//! rewrites in place to `field_address + offset` (craziiEmu `RelocatePointerField`).
//! [`relocate`] performs the same rewrite.

use selfish_bytes::{read_le, write_le, write_slice};

/// The format table, with its provenance header. Code reads this rather than carrying offsets.
const FORMAT: &str = include_str!("../../../data/agc-shader-format.tsv");

/// A single GPU register entry: an address and the value written to it.
///
/// The hardware patches the program-address registers from the code pointer; the rest are the
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
    /// The registers are the shader's, read from its bytecode by the caller; the hardware patches
    /// the program-address pair (compute program lo/hi) from the code pointer at create time.
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

    /// A geometry shader container. Stage [`STAGE_GEOMETRY`].
    #[must_use]
    pub fn geometry(shader_size: u32, target: u32, sh_registers: Vec<ShaderRegister>) -> Self {
        Self::for_stage(STAGE_GEOMETRY, shader_size, target, sh_registers)
    }

    /// An export shader container. Stage [`STAGE_EXPORT`].
    #[must_use]
    pub fn export(shader_size: u32, target: u32, sh_registers: Vec<ShaderRegister>) -> Self {
        Self::for_stage(STAGE_EXPORT, shader_size, target, sh_registers)
    }

    /// A hull shader container. Stage [`STAGE_HULL`].
    #[must_use]
    pub fn hull(shader_size: u32, target: u32, sh_registers: Vec<ShaderRegister>) -> Self {
        Self::for_stage(STAGE_HULL, shader_size, target, sh_registers)
    }

    /// A container for a given stage `type` value.
    ///
    /// The named constructors ([`compute`](Self::compute), [`pixel`](Self::pixel),
    /// [`vertex`](Self::vertex), [`geometry`](Self::geometry), [`export`](Self::export),
    /// [`hull`](Self::hull)) cover the stage values confirmed on hardware; this takes the stage
    /// byte directly.
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
    /// set to the self-relative offset the hardware relocates.
    ///
    /// # Panics
    ///
    /// If the compiled-in `data/agc-shader-format.tsv` lacks a row this needs.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        let header_size = size_of_group("header");
        let mut out = vec![0_u8; header_size];

        // The header is sized from the same table as the offsets, so every field fits.
        write_slice(&mut out, offset_of("header", "file_header"), &magic());
        write_le(&mut out, offset_of("header", "version"), version());
        write_le(
            &mut out,
            offset_of("header", "header_size"),
            as_u32(header_size),
        );
        write_le(
            &mut out,
            offset_of("header", "shader_size"),
            self.shader_size,
        );
        write_le(
            &mut out,
            offset_of("header", "embedded_constant_buffer_size_dqw"),
            self.embedded_constant_buffer_size_dqw,
        );
        write_le(&mut out, offset_of("header", "target"), self.target);
        write_le(
            &mut out,
            offset_of("header", "scratch_size_dw_per_thread"),
            self.scratch_size_dw_per_thread,
        );
        write_le(&mut out, offset_of("header", "type"), self.stage);
        write_le(
            &mut out,
            offset_of("header", "num_cx_registers"),
            as_u8(self.cx_registers.len()),
        );
        write_le(
            &mut out,
            offset_of("header", "num_sh_registers"),
            as_u8(self.sh_registers.len()),
        );

        // An empty array leaves its field zero, a null pointer.
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
/// Does nothing for an empty array: the pointer field stays zero, which the hardware reads as
/// no sub-table.
fn append_registers(out: &mut Vec<u8>, field: usize, registers: &[ShaderRegister]) {
    if registers.is_empty() {
        return;
    }
    let at = out.len();
    for register in registers {
        out.extend_from_slice(&register.offset.to_le_bytes());
        out.extend_from_slice(&register.value.to_le_bytes());
    }
    // field_address + offset = sub_table_address, so offset = at - field.
    let relative = as_u64(at).wrapping_sub(as_u64(field));
    write_le(out, field, relative);
}

/// Resolve one pointer field the way `sceAgcCreateShader` does: `field += field_address`.
///
/// Returns the absolute offset of the sub-table within `container`, or `None` for a null field.
#[must_use]
pub fn relocate(container: &[u8], field: usize) -> Option<usize> {
    let relative = read_le::<u64>(container, field)?;
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
            offset: read_le(container, at)?,
            value: read_le(container, at.checked_add(4)?)?,
        });
    }
    Some(out)
}

/// The compute stage `type` (CS). Matches `COMP_PGM_LO` (0x20c).
pub const STAGE_COMPUTE: u8 = 0;

/// The pixel (fragment) stage `type` (PS). Matches `SPI_SHADER_PGM_LO_PS` (0x08).
pub const STAGE_PIXEL: u8 = 1;

/// The vertex stage `type` (VS). Matches `SPI_SHADER_PGM_LO_VS` (0xc8).
pub const STAGE_VERTEX: u8 = 2;

/// The geometry stage `type` (GS). Matches `SPI_SHADER_PGM_LO_GS` (0x148).
pub const STAGE_GEOMETRY: u8 = 3;

/// The local shader stage `type` (LS, unfused VS half). Accepted unconditionally by `sceAgcCreateShader`.
pub const STAGE_LOCAL: u8 = 4;

/// The hull half stage `type` (HS half, unfused Hull half). Accepted unconditionally by `sceAgcCreateShader`.
pub const STAGE_HULL_HALF: u8 = 5;

/// The export shader stage `type` (ES). Matches `SPI_SHADER_PGM_LO_ES` (0x88).
pub const STAGE_EXPORT: u8 = 6;

/// The hull shader stage `type` (HS). Matches `SPI_SHADER_PGM_LO_HS` (0x108).
pub const STAGE_HULL: u8 = 7;

/// The `file_header` magic bytes, from the table.
///
/// # Panics
///
/// If the compiled-in table lacks the row.
#[must_use]
pub fn magic() -> Vec<u8> {
    bytes_of("header", "file_header")
}

/// The `version` value, from the table.
///
/// # Panics
///
/// If the compiled-in table lacks the row.
#[must_use]
pub fn version() -> u32 {
    u32::try_from(number_of("header", "version"))
        .unwrap_or_else(|_| panic!("header/version does not fit in a u32"))
}

/// One field's offset within its group.
///
/// # Panics
///
/// If the compiled-in table lacks the row.
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{
        Container, STAGE_COMPUTE, STAGE_EXPORT, STAGE_GEOMETRY, STAGE_HULL, STAGE_HULL_HALF,
        STAGE_LOCAL, STAGE_PIXEL, STAGE_VERTEX, ShaderRegister, magic, offset_of, registers_at,
        relocate, version,
    };

    /// The compute program-address registers, from craziiEmu: lo at 0x20C, hi at 0x20D, with
    /// placeholder values the hardware patches from the code pointer.
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

    /// The header carries the magic `1234` and version `0x18` the readers validate.
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

    /// The table's header offsets match the ones an emulator pins with a `static_assert`.
    #[test]
    fn the_offsets_are_the_pinned_ones() {
        // `prosper`'s `static_assert`.
        assert_eq!(offset_of("header", "user_data"), 0x08);
        assert_eq!(offset_of("header", "code"), 0x10);
        assert_eq!(offset_of("header", "specials"), 0x28);
        assert_eq!(offset_of("header", "type"), 0x5a);
        assert_eq!(offset_of("header", "num_sh_registers"), 0x5c);
        assert_eq!(offset_of("header", "shader_size"), 0x44);
        assert_eq!(offset_of("header", "target"), 0x4c);
    }

    /// The scalar metadata lands at the table's offsets.
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

    /// Every stage's container differs from compute only in the `type` byte.
    #[test]
    fn all_eight_stages_differ_from_compute_only_in_the_type_byte() {
        let regs = compute_regs();
        let compute = Container::compute(256, 0x0E, regs.clone()).build();
        let pixel = Container::pixel(256, 0x0E, regs.clone()).build();
        let vertex = Container::vertex(256, 0x0E, regs.clone()).build();
        let geometry = Container::geometry(256, 0x0E, regs.clone()).build();
        let local = Container::for_stage(STAGE_LOCAL, 256, 0x0E, regs.clone()).build();
        let hull_half = Container::for_stage(STAGE_HULL_HALF, 256, 0x0E, regs.clone()).build();
        let export = Container::export(256, 0x0E, regs.clone()).build();
        let hull = Container::hull(256, 0x0E, regs).build();

        assert_eq!(compute[0x5a], STAGE_COMPUTE, "compute type = 0");
        assert_eq!(pixel[0x5a], STAGE_PIXEL, "pixel type = 1");
        assert_eq!(vertex[0x5a], STAGE_VERTEX, "vertex type = 2");
        assert_eq!(geometry[0x5a], STAGE_GEOMETRY, "geometry type = 3");
        assert_eq!(local[0x5a], STAGE_LOCAL, "local type = 4");
        assert_eq!(hull_half[0x5a], STAGE_HULL_HALF, "hull_half type = 5");
        assert_eq!(export[0x5a], STAGE_EXPORT, "export type = 6");
        assert_eq!(hull[0x5a], STAGE_HULL, "hull type = 7");

        let differs = |a: &[u8], b: &[u8]| -> Vec<usize> {
            (0..a.len().min(b.len()))
                .filter(|i| a[*i] != b[*i])
                .collect()
        };
        for (name, stage_buf) in [
            ("pixel", pixel),
            ("vertex", vertex),
            ("geometry", geometry),
            ("local", local),
            ("hull_half", hull_half),
            ("export", export),
            ("hull", hull),
        ] {
            assert_eq!(
                differs(&compute, &stage_buf),
                vec![0x5a],
                "{name} vs compute: only type byte differs"
            );
        }
    }

    /// `for_stage` writes the stage byte it is handed unchanged.
    #[test]
    fn for_stage_writes_any_type_value_it_is_given() {
        let built = Container::for_stage(4, 256, 0x0E, compute_regs()).build();
        assert_eq!(built[0x5a], 4, "for_stage writes the type it is handed");
    }

    /// Registers survive a build and relocate round trip through their self-relative pointer.
    #[test]
    fn a_self_relative_pointer_relocates_to_its_sub_table_and_reads_back() {
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

    /// An empty register array leaves its pointer field null.
    #[test]
    fn an_empty_register_array_leaves_a_null_pointer() {
        let built = Container::compute(256, 0x0E, compute_regs()).build();
        let cx = offset_of("header", "cx_registers");
        assert_eq!(&built[cx..cx + 8], &[0_u8; 8], "cx_registers is null");
        assert!(relocate(&built, cx).is_none());
    }

    /// The first sub-table starts at or after `header_size`.
    #[test]
    fn the_header_region_is_clear_of_the_sub_tables() {
        let built = Container::compute(256, 0x0E, compute_regs()).build();
        let header_size = u32::from_le_bytes(built[0x40..0x44].try_into().unwrap()) as usize;
        let base = relocate(&built, offset_of("header", "sh_registers")).unwrap();
        assert!(
            base >= header_size,
            "sub-table {base:#x} is at/after header_size {header_size:#x}"
        );
    }
}
