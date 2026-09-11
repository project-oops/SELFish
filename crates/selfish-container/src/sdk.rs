#![allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::unreadable_literal,
    clippy::missing_errors_doc,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::must_use_candidate,
    clippy::redundant_closure_for_method_calls
)]

//! Target SDK version dictionary and procparam stamping for `SELFish`.
//!
//! Provides human-readable SDK names and aliases mapped to Orbis and Prospero (PPR)
//! packed 32-bit versions, validates target generation compatibility, and stamps
//! `PT_SCE_PROCPARAM` directly into ELF binaries.

use selfish_abi::Generation;
use std::collections::BTreeMap;
use std::path::Path;

/// Embedded default dictionary from `data/sdk-versions.toml`.
const DEFAULT_SDK_TOML: &str = include_str!("../../../data/sdk-versions.toml");

/// A known SDK definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkEntry {
    /// Canonical version name (e.g. "2.000.009").
    pub name: String,
    /// Hardware generation (4 for Orbis, 5 for Prospero).
    pub generation: u8,
    /// Friendly alias (e.g. "prospero").
    pub alias: Option<String>,
    /// Packed Orbis SDK version (0xMMmmppbb).
    pub orbis_sdk: u32,
    /// Packed Prospero (PPR) SDK version (0xMMmmppbb).
    pub ppr_sdk: u32,
    /// Minimum console firmware required.
    pub min_firmware: String,
    /// Human-readable description.
    pub description: String,
}

/// Target SDK configuration for building an executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetSdk {
    /// Hardware generation.
    pub generation: Generation,
    /// Packed Orbis SDK version.
    pub orbis_sdk: u32,
    /// Packed Prospero (PPR) SDK version.
    pub ppr_sdk: u32,
}

impl TargetSdk {
    /// Create a target SDK directly from generation and packed values.
    #[must_use]
    pub const fn new(generation: Generation, orbis_sdk: u32, ppr_sdk: u32) -> Self {
        Self {
            generation,
            orbis_sdk,
            ppr_sdk,
        }
    }

    /// Default target SDK for a given generation.
    #[must_use]
    pub fn default_for(generation: Generation) -> Self {
        match generation {
            Generation::Previous => Self {
                generation,
                orbis_sdk: 0x08008011,
                ppr_sdk: 0x00000000,
            },
            Generation::Current => Self {
                generation,
                orbis_sdk: 0x08050001,
                ppr_sdk: 0x02000009, // Universal safe Prospero baseline (SDK 2.00)
            },
        }
    }
}

/// A parsed dictionary of SDK versions.
#[derive(Debug, Clone, Default)]
pub struct SdkDictionary {
    entries: BTreeMap<String, SdkEntry>,
}

impl SdkDictionary {
    /// Load dictionary from the embedded TOML data.
    #[must_use]
    pub fn embedded() -> Self {
        Self::parse_toml(DEFAULT_SDK_TOML).unwrap_or_default()
    }

    /// Load dictionary from an external file path, falling back to embedded.
    pub fn load_or_embedded(path: Option<&Path>) -> Self {
        if let Some(p) = path {
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(dict) = Self::parse_toml(&content) {
                    return dict;
                }
            }
        }
        Self::embedded()
    }

    /// Parse a minimal TOML string containing `[sdks."<version>"]` blocks.
    pub fn parse_toml(content: &str) -> Result<Self, String> {
        let mut entries = BTreeMap::new();
        let mut current_name: Option<String> = None;
        let mut cur_gen: u8 = 5;
        let mut cur_alias: Option<String> = None;
        let mut cur_orbis: u32 = 0;
        let mut cur_ppr: u32 = 0;
        let mut cur_min_fw: String = String::new();
        let mut cur_desc: String = String::new();

        let flush = |entries: &mut BTreeMap<String, SdkEntry>,
                     name: &Option<String>,
                     generation: u8,
                     alias: Option<String>,
                     orbis: u32,
                     ppr: u32,
                     min_fw: String,
                     desc: String| {
            if let Some(n) = name {
                entries.insert(
                    n.clone(),
                    SdkEntry {
                        name: n.clone(),
                        generation,
                        alias,
                        orbis_sdk: orbis,
                        ppr_sdk: ppr,
                        min_firmware: min_fw,
                        description: desc,
                    },
                );
            }
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with("[sdks.\"") && line.ends_with("\"]") {
                flush(
                    &mut entries,
                    &current_name,
                    cur_gen,
                    cur_alias.take(),
                    cur_orbis,
                    cur_ppr,
                    std::mem::take(&mut cur_min_fw),
                    std::mem::take(&mut cur_desc),
                );
                let name = &line[7..line.len() - 2];
                current_name = Some(name.to_string());
                cur_gen = 5;
                cur_orbis = 0;
                cur_ppr = 0;
                continue;
            }

            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"');
                match key {
                    "generation" => {
                        cur_gen = val.parse().unwrap_or(5);
                    }
                    "alias" => {
                        cur_alias = Some(val.to_string());
                    }
                    "orbis_sdk" => {
                        cur_orbis = parse_packed_hex(val);
                    }
                    "ppr_sdk" => {
                        cur_ppr = parse_packed_hex(val);
                    }
                    "min_firmware" => {
                        cur_min_fw = val.to_string();
                    }
                    "description" => {
                        cur_desc = val.to_string();
                    }
                    _ => {}
                }
            }
        }

        flush(
            &mut entries,
            &current_name,
            cur_gen,
            cur_alias.take(),
            cur_orbis,
            cur_ppr,
            cur_min_fw,
            cur_desc,
        );

        Ok(Self { entries })
    }

    /// Resolve an SDK identifier (friendly name, alias, or hex) for a target generation.
    pub fn resolve(&self, input: &str, target_gen: Generation) -> Result<TargetSdk, String> {
        let trimmed = input.trim();

        // 1. Direct hex format: 0xMMmmppbb
        if let Some(hex_str) = trimmed.strip_prefix("0x") {
            if let Ok(val) = u32::from_str_radix(hex_str, 16) {
                return Ok(match target_gen {
                    Generation::Previous => TargetSdk::new(target_gen, val, 0),
                    Generation::Current => TargetSdk::new(target_gen, 0x08050001, val),
                });
            }
        }

        // 2. Match against name or alias in dictionary
        for entry in self.entries.values() {
            let matches_name = entry.name.eq_ignore_ascii_case(trimmed);
            let matches_alias = entry.alias.as_ref().is_some_and(|a| {
                a.split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case(trimmed))
            }) || match trimmed.to_ascii_lowercase().as_str() {
                "prospero" | "prospero-default" => entry.name == "2.000.009",
                "trinity" => entry.name == "11.600.005",
                "orbis" | "orbis-default" | "neo" => entry.name == "8.008.011",
                _ => false,
            };

            if matches_name || matches_alias {
                let entry_gen = match entry.generation {
                    4 => Generation::Previous,
                    5 => Generation::Current,
                    other => return Err(format!("Unknown generation {other} in SDK entry")),
                };

                // Enforce generation constraints:
                if entry_gen != target_gen
                    && entry.ppr_sdk != 0
                    && target_gen == Generation::Previous
                {
                    return Err(format!(
                        "SDK target '{input}' is for Prospero-generation and cannot be used with an Orbis-generation target"
                    ));
                }

                return Ok(TargetSdk::new(target_gen, entry.orbis_sdk, entry.ppr_sdk));
            }
        }

        // 3. Fallback: Parse dotted format (e.g. "2.000.009" or "2.00")
        if let Some(packed) = parse_dotted_version(trimmed) {
            return Ok(match target_gen {
                Generation::Previous => TargetSdk::new(target_gen, packed, 0),
                Generation::Current => TargetSdk::new(target_gen, 0x08050001, packed),
            });
        }

        Err(format!(
            "Unrecognized SDK version '{input}'. Use a known alias (e.g. 'prospero', 'orbis', 'trinity', 'neo'), \
             dotted version (e.g. '2.000.009'), or hex (e.g. '0x02000009')"
        ))
    }
}

/// Convert a packed hex string like "02.000.009" or "02000009" or "0x02000009" to u32.
fn parse_packed_hex(val: &str) -> u32 {
    let clean: String = val.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    u32::from_str_radix(&clean, 16).unwrap_or(0)
}

/// Convert a dotted version string like "2.00" or "2.000.009" into packed 32-bit hex.
fn parse_dotted_version(val: &str) -> Option<u32> {
    let parts: Vec<&str> = val.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }

    let mut bytes = [0u8; 4];
    for (i, p) in parts.iter().enumerate() {
        let clean: String = p.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if clean.is_empty() {
            return None;
        }
        let num = u8::from_str_radix(&clean, 16).ok()?;
        bytes[i] = num;
    }

    Some(u32::from_be_bytes(bytes))
}

/// Patch `PT_SCE_PROCPARAM` segment inside ELF binary bytes.
///
/// Returns true if a `PT_SCE_PROCPARAM` segment was located and stamped with the SDK versions.
#[allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]
pub fn patch_elf_procparam(elf_bytes: &mut [u8], target_sdk: TargetSdk) -> bool {
    if elf_bytes.len() < 64 || &elf_bytes[0..4] != b"\x7fELF" {
        return false;
    }

    let phoff = u64::from_le_bytes(elf_bytes[32..40].try_into().unwrap_or([0; 8])) as usize;
    let phnum = u16::from_le_bytes(elf_bytes[56..58].try_into().unwrap_or([0; 2])) as usize;
    let phentsize = u16::from_le_bytes(elf_bytes[54..56].try_into().unwrap_or([0; 2])) as usize;

    if phentsize < 56 {
        return false;
    }

    for i in 0..phnum {
        let entry_off = phoff + i * phentsize;
        if entry_off + 56 > elf_bytes.len() {
            break;
        }

        let p_type = u32::from_le_bytes(
            elf_bytes[entry_off..entry_off + 4]
                .try_into()
                .unwrap_or([0; 4]),
        );
        if p_type == 0x61000001 {
            // Found PT_SCE_PROCPARAM
            let p_offset = u64::from_le_bytes(
                elf_bytes[entry_off + 8..entry_off + 16]
                    .try_into()
                    .unwrap_or([0; 8]),
            ) as usize;
            let p_filesz = u64::from_le_bytes(
                elf_bytes[entry_off + 32..entry_off + 40]
                    .try_into()
                    .unwrap_or([0; 8]),
            ) as usize;

            if p_offset + 0x18 <= elf_bytes.len() && p_filesz >= 0x20 {
                // Verify magic 'ORBI' at offset 0x08
                if &elf_bytes[p_offset + 8..p_offset + 12] == b"ORBI" {
                    // Update sdk_version at +0x10 and sdk_version_second at +0x14
                    elf_bytes[p_offset + 0x10..p_offset + 0x14]
                        .copy_from_slice(&target_sdk.orbis_sdk.to_le_bytes());
                    elf_bytes[p_offset + 0x14..p_offset + 0x18]
                        .copy_from_slice(&target_sdk.ppr_sdk.to_le_bytes());
                    return true;
                }
            }
        } else if p_type == 0x61000002 {
            // Found PT_SCE_MODULE_PARAM
            let p_offset = u64::from_le_bytes(
                elf_bytes[entry_off + 8..entry_off + 16]
                    .try_into()
                    .unwrap_or([0; 8]),
            ) as usize;
            let p_filesz = u64::from_le_bytes(
                elf_bytes[entry_off + 32..entry_off + 40]
                    .try_into()
                    .unwrap_or([0; 8]),
            ) as usize;

            if p_offset + 0x18 <= elf_bytes.len() && p_filesz >= 0x20 {
                // Update sdk_version at +0x10 and sdk_version_second at +0x14
                elf_bytes[p_offset + 0x10..p_offset + 0x14]
                    .copy_from_slice(&target_sdk.orbis_sdk.to_le_bytes());
                elf_bytes[p_offset + 0x14..p_offset + 0x18]
                    .copy_from_slice(&target_sdk.ppr_sdk.to_le_bytes());
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_dictionary_parses() {
        let dict = SdkDictionary::embedded();
        assert!(
            !dict.entries.is_empty(),
            "Embedded dictionary should have entries"
        );

        let native = dict
            .resolve("prospero", Generation::Current)
            .expect("prospero must resolve");
        assert_eq!(native.ppr_sdk, 0x02000009);
        assert_eq!(native.orbis_sdk, 0x08050001);

        let current = dict
            .resolve("prospero-current", Generation::Current)
            .expect("prospero-current must resolve");
        assert_eq!(current.ppr_sdk, 0x11600005);

        let compat = dict
            .resolve("orbis", Generation::Previous)
            .expect("orbis must resolve");
        assert_eq!(compat.orbis_sdk, 0x08008011);
        assert_eq!(compat.ppr_sdk, 0);
    }

    #[test]
    fn test_generation_mismatch_rejected() {
        let dict = SdkDictionary::embedded();
        let err = dict.resolve("prospero", Generation::Previous);
        assert!(
            err.is_err(),
            "Prospero SDK must be rejected for Orbis target"
        );
    }

    #[test]
    fn test_dotted_and_hex_resolution() {
        let dict = SdkDictionary::embedded();
        let from_hex = dict.resolve("0x02000009", Generation::Current).unwrap();
        assert_eq!(from_hex.ppr_sdk, 0x02000009);

        let from_dotted = dict.resolve("2.000.009", Generation::Current).unwrap();
        assert_eq!(from_dotted.ppr_sdk, 0x02000009);
    }
}
