//! Assembling a package, as far as what is established allows.
//!
//! Everything derivable is derived. Everything cited is written. **Everything unknown must be
//! supplied by the caller or the build refuses**, because the alternative is a package that is
//! wrong in a way a console acts on rather than rejects.
//!
//! # What this produces on its own
//!
//! The whole header, the entry table, and eight entry contents:
//!
//! | entry | how |
//! |---|---|
//! | `0x1` | computed - one SHA-256 per entry (D034) |
//! | `0x100` | computed - the entry table again (D034) |
//! | `0x80` | computed as far as it is known: the image digest and the `param.sfo` digest (D036) |
//! | `0x1002` | computed - four bytes of SHA-256 per block of the finished package |
//! | `0x400`, `0x401` | computed - the licences, signed under the debug RIF keyset (D047) |
//! | `0x10`, `0x20` | computed - the key blobs, reproduced byte-for-byte from real packages (D054) |
//! | `0x409` | eight kilobytes of zero, which is what every sample holds |
//!
//! # What it demands, and why that is not a gap
//!
//! Five entries are the **title's** content rather than the format's: `param.sfo`, the icon,
//! the playgo pair and the entry name table. A library that generated those would be inventing
//! the title. They are required inputs and the build names every missing one.
//!
//! # What it still cannot fill
//!
//! Three thirty-two-byte slots of `0x80` digest something that has never been found anywhere
//! in a package. They are left zero and **reported** on the builder's output rather than
//! hidden, so `is_complete()` says `false` and a caller can see exactly what is missing.
//!
//! # What it signs, and what it does not
//!
//! Nothing here claims to be the vendor. The licences are signed under the published debug RIF
//! keyset, which asserts "this is a debug licence" and is true (D047). The key blobs are
//! *wrapped* under public keys - a public key cannot unwrap, so producing them gains no ability
//! to read anything (D054). The container's own signature area stays zero. (principle 6)

use sha2::{Digest, Sha256};

use crate::derive::{self, DIGEST};
use crate::{ENTRY_SIZE, Entry, MAGIC, entry_id, keys};

/// Offset of the entry count in the header.
const COUNT_AT: usize = 0x10;
/// Offset of the entry table's own offset.
const TABLE_AT: usize = 0x18;
/// Offset of the content id.
const CONTENT_ID_AT: usize = 0x40;
/// How long a content id is.
const CONTENT_ID_LEN: usize = 36;
/// Offset of the image offset, as a 64-bit value.
const IMAGE_AT: usize = 0x410;

/// The rest of the header, which a console reads and this crate wrote none of until now.
///
/// # Why this was worth finding before a hardware trip and not after
///
/// A package whose header says its image is zero bytes long is not a package with a subtle
/// problem - there is nothing for a console to mount. Every field below was measured out of a
/// real package (`xxd -s 0x400`) and matches `LibOrbisPkg@6434772`'s writer offset for offset,
/// including two constants nobody would guess: a version date of `0x20161020` and a version
/// hash of `0x1738551`, both of which appear verbatim in the sample. (D056)
mod header {
    /// Package flags.
    pub(super) const FLAGS: usize = 0x04;
    /// Unnamed, and `0xF` in every package examined.
    pub(super) const UNK_0C: usize = 0x0C;
    /// How many entries the sc region describes.
    pub(super) const SC_ENTRY_COUNT: usize = 0x14;
    /// The entry count again, as sixteen bits.
    pub(super) const ENTRY_COUNT_2: usize = 0x16;
    /// How much of the entry data is the main region.
    pub(super) const MAIN_ENTRY_DATA_SIZE: usize = 0x1C;
    /// Where the body begins.
    pub(super) const BODY_OFFSET: usize = 0x20;
    /// How long the body is.
    pub(super) const BODY_SIZE: usize = 0x28;
    /// What kind of DRM the title declares.
    pub(super) const DRM_TYPE: usize = 0x70;
    /// What kind of content it is.
    pub(super) const CONTENT_TYPE: usize = 0x74;
    /// Content flags.
    pub(super) const CONTENT_FLAGS: usize = 0x78;
    /// Promote size.
    pub(super) const PROMOTE_SIZE: usize = 0x7C;
    /// A date, constant across every package examined.
    pub(super) const VERSION_DATE: usize = 0x80;
    /// A hash, likewise constant.
    pub(super) const VERSION_HASH: usize = 0x84;
    /// The DRM type's own version.
    pub(super) const EKC_VERSION: usize = 0x9C;
    /// Unnamed, and `1` in every package examined.
    pub(super) const UNK_400: usize = 0x400;
    /// How many images the package carries.
    pub(super) const IMAGE_COUNT: usize = 0x404;
    /// Flags describing the image.
    pub(super) const PFS_FLAGS: usize = 0x408;
    /// **How long the image is.** Zero here and there is nothing to mount.
    pub(super) const IMAGE_SIZE: usize = 0x418;
    /// Where the mount image begins.
    pub(super) const MOUNT_IMAGE_OFFSET: usize = 0x420;
    /// How long the mount image is.
    pub(super) const MOUNT_IMAGE_SIZE: usize = 0x428;
    /// The whole package's length, which is the file's length.
    pub(super) const PACKAGE_SIZE: usize = 0x430;
    /// How much of the image is signed.
    pub(super) const SIGNED_SIZE: usize = 0x438;
    /// How much of it a console caches.
    pub(super) const CACHE_SIZE: usize = 0x43C;
    /// A digest of the whole image.
    pub(super) const IMAGE_DIGEST: usize = 0x440;
    /// A digest of the image's first signed region.
    pub(super) const SIGNED_DIGEST: usize = 0x460;
    /// SHA-256 of the five SC entry bodies (`0x10,0x20,0x80,0x100,0x1`). Measured 1/1.
    pub(super) const SC_ENTRIES1_HASH: usize = 0x100;
    /// SHA-256 of four SC entry bodies, `0x100` truncated to `sc_entry_count * 0x20`. Measured 1/1.
    pub(super) const SC_ENTRIES2_HASH: usize = 0x120;
    /// A digest of the digest table, entry `0x1`. Measured: matches `sha256(entry 0x1)`.
    ///
    /// One of four thirty-two byte slots at `0x100`-`0x17F` that this crate left entirely zero
    /// while every real package fills them. A console fetched a package built here, parsed it,
    /// and refused it with `0x80f00101` - content rejected, not transport.
    pub(super) const DIGEST_TABLE_DIGEST: usize = 0x140;
    /// A digest of the body, the region [`BODY_OFFSET`]..[`BODY_SIZE`] describes.
    ///
    /// Measured: matches `sha256(file[0x2000..0x2000 + 0x7E000])`.
    ///
    /// [`BODY_OFFSET`]: self::BODY_OFFSET
    /// [`BODY_SIZE`]: self::BODY_SIZE
    pub(super) const BODY_DIGEST: usize = 0x160;
}

/// The cache size every real package declares, and the default this crate writes.
///
/// Public because a caller has to be able to compare its own image against it: a package whose
/// inner filesystem is smaller than this cannot mount, and the check belongs where the inner size
/// is known. See [`Builder::cache_size`]. (D070)
pub const DEFAULT_CACHE_SIZE: u32 = 0xD_0000;

/// Constants the header carries that are the same in every package examined.
mod header_value {
    /// The only flag a fake package sets.
    pub(super) const FLAGS: u32 = 0x01;
    /// Content flags. Zero reads as an unconfigured package; this is the value a real homebrew
    /// title carries, observed in a working package on a console (an oracle, not a source).
    pub(super) const CONTENT_FLAGS: u32 = 0x0A00_0000;
    /// How many SC entries a package declares at `0x14`. Six in all three packages examined,
    /// regardless of total entry count (14, 23, 14) - so it counts the format own entries,
    /// not the title ones. (measured 3/3)
    pub(super) const SC_ENTRY_COUNT: u16 = 6;
    /// Unnamed.
    pub(super) const UNK_0C: u32 = 0x0F;
    /// One image.
    pub(super) const IMAGE_COUNT: u32 = 1;
    /// Unnamed, before the image fields.
    pub(super) const UNK_400: u32 = 1;
    /// Flags over the image.
    pub(super) const PFS_FLAGS: u64 = 0x8000_0000_0000_03CC;
    /// How much of the image is covered by the signed digest.
    pub(super) const SIGNED_SIZE: u32 = 0x10000;
    /// How much a console caches.
    pub(super) const CACHE_SIZE: u32 = super::DEFAULT_CACHE_SIZE;
    /// Where the body begins, in every package examined.
    pub(super) const BODY_OFFSET: u64 = 0x2000;
    /// How long it is, in every package examined.
    pub(super) const BODY_SIZE: u64 = 0x7E000;
    /// A date. Constant, and not the build date of anything here.
    pub(super) const VERSION_DATE: u32 = 0x2016_1020;
    /// A hash. Likewise constant and likewise not derived from anything.
    pub(super) const VERSION_HASH: u32 = 0x0173_8551;
    /// The DRM type's version.
    pub(super) const EKC_VERSION: u32 = 1;
}

/// Where the image goes.
///
/// Fixed at `0x80000` rather than packed in behind the entries. Real packages put it here, and
/// so does `LibOrbisPkg@6434772`; a console has never been observed reading it from anywhere else, and
/// the saving from moving it would be half a megabyte. (D056)
const IMAGE_OFFSET: usize = 0x80000;
/// Where the entry table begins, which is **fixed** rather than merely clear of the header.
///
/// This was `0x1000`, on the reasoning that the table starts past the last header field this
/// crate knows about and "a builder only has to not collide". A console refuted that: a package
/// built here was refused by `scePlayGoCoreGetRawContentInfo` with `0x80f00101`, and bisecting a
/// working package against ours - copying our bytes into it a region at a time - narrowed the
/// rejection to **four bytes**, the table offset at `0x18`. Nothing else in the header mattered;
/// with the real value restored the same package parsed.
///
/// `0x2A80` in all three packages examined, whatever their entry count. So it is a constant of
/// the format, not an arithmetic result, and a package whose table is merely *somewhere valid*
/// is one a console will not read. (measured 3/3)
const HEADER_RESERVED: usize = 0x2A80;

/// Entries this module fills in by itself.
///
/// A caller supplying one of these is refused rather than silently overridden: two sources for
/// one entry is how a digest table stops matching the entries it describes.
const COMPUTED: [u32; 8] = [
    derive::entry::DIGESTS,
    derive::entry::TABLE_COPY,
    derive::entry::MANIFEST,
    derive::entry::PLAYGO_CHUNK_SHA,
    entry_id::LICENSE_DAT,
    entry_id::LICENSE_INFO,
    // The two key blobs. These used to be demanded from the caller, and a caller with nothing
    // to hand supplied zeros - which produces a package that parses, extracts and passes every
    // test here, and that a console cannot open, because the filesystem key is inside them.
    // Both are now reproduced byte-for-byte from real packages (D054).
    entry_id::ENTRY_KEYS,
    entry_id::IMAGE_KEY,
];

/// A package under construction.
#[derive(Debug, Default)]
pub struct Builder {
    content_id: String,
    passcode: Option<Vec<u8>>,
    supplied: Vec<(u32, Vec<u8>)>,
    image: Vec<u8>,
    drm_type: u16,
    content_type: u16,
    sku_flag: u16,
    cache_size: Option<u32>,
}

impl Builder {
    /// Start one, with the defaults an installable homebrew package needs.
    ///
    /// A package left at zero for these describes nothing: an installer reads `content_type = 0`
    /// as "not a title I will register" and reports an empty content id, type and platform for
    /// the whole package - which is exactly what a fake package built with zeros hit on
    /// hardware. So the defaults are the values a real homebrew title carries, deterministically,
    /// rather than zeros that pass the writer and fail the console. A caller building something
    /// other than an application - additional content, a patch - overrides them with [`kind`].
    ///
    /// `0x0F` is the free/fake DRM type; `0x1A` is `CONTENT_TYPE_GD`, an application's own data.
    ///
    /// [`kind`]: Self::kind
    #[must_use]
    pub fn new() -> Self {
        Self {
            drm_type: 0x0F,
            content_type: 0x1A,
            ..Self::default()
        }
    }

    /// The content id, such as `UP0000-PPSA01650_00-YOUTUBE000000000`.
    #[must_use]
    pub fn content_id(mut self, id: &str) -> Self {
        id.clone_into(&mut self.content_id);
        self
    }

    /// The passcode the package is keyed with.
    ///
    /// Defaults to [`keys::FAKE_PASSCODE`], which is what community tooling uses and what two
    /// of the three packages examined here were built with. It is an *input*: nothing can
    /// recover it from a finished package, so a caller choosing its own must remember it.
    ///
    /// It reaches further than it looks. The filesystem key, both key blobs and the encryption
    /// over the encrypted entries all derive from it, and they have to move together - a
    /// package whose entries are keyed one way and whose blobs point another cannot be opened
    /// by anything. (D055)
    #[must_use]
    pub fn passcode(mut self, passcode: &[u8]) -> Self {
        self.passcode = Some(passcode.to_vec());
        self
    }

    /// The filesystem image, already built and encrypted.
    ///
    /// Taken whole rather than assembled here, because a package holds an image and does not
    /// care how one is made. `selfish_pfs::write`, `selfish_pfs::pfsc` and `selfish_pfs::outer`
    /// now produce one from a tree of files; the note that once stood here saying they could
    /// not is gone with the gap it described.
    #[must_use]
    pub fn image(mut self, image: Vec<u8>) -> Self {
        self.image = image;
        self
    }

    /// How much of the image a console may cache, overriding the constant every real package
    /// carries.
    ///
    /// # Why this is not simply a constant
    ///
    /// It was one - `0xD0000`, which is what all three real packages hold. All three are also
    /// tens of megabytes, and a constant measured only from large samples turned out to be a
    /// constraint nobody had stated: a console **refuses an image whose inner filesystem is
    /// smaller than the cache the header declares**, with
    /// `sceFsMountGamePkg ***ERR*** Failed to enable GDDR5 cache` and `EINVAL`, after the outer
    /// image has already mounted and `pfs_image.dat` has already opened.
    ///
    /// A minimal package hit exactly that: an inner image of `0xB0000` against a declared
    /// `0xD0000`. Padding the inner image past the declared size cleared the error outright,
    /// which is what establishes the rule rather than a guess about it. (measured)
    ///
    /// So a caller that knows its inner image is small can say so. What the field *means* beyond
    /// "not larger than the thing being cached" is still unknown, and nothing here pretends
    /// otherwise - this is a ceiling, not a formula.
    #[must_use]
    pub fn cache_size(mut self, bytes: u32) -> Self {
        self.cache_size = Some(bytes);
        self
    }

    /// What the title is, for the licence.
    ///
    /// Override the content kind. These describe the *content* - game, patch or add-on - so a
    /// caller that is not building an application overrides the application defaults [`new`] sets
    /// here. What must not happen is zeros reaching the header: an installer reads those as a
    /// package it will not register, so the default is a working application, not nothing.
    ///
    /// [`new`]: Self::new
    #[must_use]
    pub const fn kind(mut self, drm_type: u16, content_type: u16, sku_flag: u16) -> Self {
        self.drm_type = drm_type;
        self.content_type = content_type;
        self.sku_flag = sku_flag;
        self
    }

    /// Supply an entry this crate cannot compute.
    #[must_use]
    pub fn entry(mut self, id: u32, contents: Vec<u8>) -> Self {
        self.supplied.push((id, contents));
        self
    }

    /// Lay the package out and emit it.
    ///
    /// # Errors
    ///
    /// If a computed entry was also supplied, if the content id is too long, or if a required
    /// entry is missing. The missing-entry error **names every one**, because the answer is
    /// always to go and find them and a count does not say which.
    //
    // Long because it is one linear assembly: validate, lay the bodies out in rank order,
    // build the entry table, then write the digests that can only be computed once everything
    // before them is in place. Splitting it would scatter that order across call sites, and the
    // order is the part that has to be right - several of these digests read bytes that are only
    // correct after the step before wrote them.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<Built, WriteError> {
        if self.content_id.len() > CONTENT_ID_LEN {
            return Err(WriteError::ContentIdTooLong(self.content_id.len()));
        }
        for (id, _) in &self.supplied {
            if COMPUTED.contains(id) {
                return Err(WriteError::AlreadyComputed(*id));
            }
        }

        // Every entry the samples carry, in ascending id - which is the order they appear in,
        // and the order the digest table at `0x1` is indexed by.
        let mut contents: Vec<(u32, Vec<u8>)> = self.supplied.clone();
        contents.push((entry_id::PARAM_SFO_ZEROS, vec![0_u8; ZEROS_LEN]));
        for id in COMPUTED {
            contents.push((id, Vec::new()));
        }
        contents.sort_by_key(|(id, _)| *id);
        contents.dedup_by_key(|(id, _)| *id);

        let missing: Vec<u32> = entry_id::ALWAYS_PRESENT
            .iter()
            .copied()
            .filter(|id| !contents.iter().any(|(held, _)| held == id))
            .collect();
        if !missing.is_empty() {
            return Err(WriteError::Missing(missing));
        }

        let count = contents.len();
        self.size_computed_bodies(&mut contents, count)?;

        // Lay the bodies out from body_offset in body layout rank order.
        contents.sort_by_key(|(id, _)| layout_rank(*id));

        let mut at = usize::try_from(header_value::BODY_OFFSET).unwrap_or(0x2000);
        let mut table_at = HEADER_RESERVED;
        let mut entries = Vec::with_capacity(count);
        for (id, body) in &contents {
            let offset = u32::try_from(at).map_err(|_| WriteError::TooLarge)?;
            let size = u32::try_from(body.len()).map_err(|_| WriteError::TooLarge)?;
            if *id == derive::entry::TABLE_COPY {
                table_at = at; // the metas entry IS the entry table
            }
            // One arm per entry id, including where two ids carry the same flags. This is a
            // format table written as code, and the rows are the point: merging `0x10`, `0x80`
            // and `0x100` because they happen to agree today would hide which ids were measured
            // and turn a later divergence into a surprise. `data/` states the same rule for the
            // tables it holds; this is the same table with a `match` around it.
            #[allow(clippy::match_same_arms)]
            let (flags1, flags2) = match *id {
                0x0001 => (0x4000_0000, 0),
                0x0010 => (0x6000_0000, 0),
                0x0020 => (0xE000_0000, keys::IMAGE_KEY_INDEX << 12),
                0x0080 => (0x6000_0000, 0),
                0x0100 => (0x6000_0000, 0),
                0x0200 => (0x4000_0000, 0),
                entry_id::LICENSE_DAT => (keys::FLAG_ENCRYPTED, LICENCE_KEY_INDEX << 12),
                entry_id::LICENSE_INFO => (keys::FLAG_ENCRYPTED, LICENCE_INFO_KEY_INDEX << 12),
                _ => (0, 0),
            };
            let name_offset = resolve_name_offset(*id, &contents);
            entries.push(Entry {
                id: *id,
                name_offset,
                flags1,
                flags2,
                offset,
                size,
            });
            // Each body follows the last, rounded up to sixteen bytes.
            at = at
                .checked_add(body.len())
                .ok_or(WriteError::TooLarge)?
                .saturating_add(0xF)
                & !0xF;
        }

        // The Entry Table at 0x2A80 (and entry 0x100) MUST have its records sorted by entry ID:
        entries.sort_by_key(|e| e.id);

        if at > IMAGE_OFFSET {
            return Err(WriteError::TooLarge);
        }
        let image_at = IMAGE_OFFSET;

        encrypt_licences(
            &mut contents,
            &entries,
            &self.content_id,
            self.passcode_bytes(),
        )?;

        let mut buffer = vec![0_u8; image_at];
        buffer.extend_from_slice(&self.image);
        let playgo = derive::playgo_chunk_sha(&buffer, image_at);
        set(&mut contents, derive::entry::PLAYGO_CHUNK_SHA, playgo);

        let mut gaps = Vec::new();
        set(
            &mut contents,
            derive::entry::TABLE_COPY,
            derive::entry_table_copy(&entries),
        );
        let manifest = self.manifest(&contents, &mut gaps);
        set(&mut contents, derive::entry::MANIFEST, manifest);
        let bodies: Vec<&[u8]> = entries
            .iter()
            .map(|e| {
                contents
                    .iter()
                    .find(|(id, _)| *id == e.id)
                    .map_or(&[] as &[u8], |(_, body)| body.as_slice())
            })
            .collect();
        let self_slot = entries
            .iter()
            .position(|e| e.id == derive::entry::DIGESTS)
            .unwrap_or(0);
        let digests = derive::digest_table(&bodies, self_slot);
        set(&mut contents, derive::entry::DIGESTS, digests);

        Self::emit(
            buffer,
            &self.content_id,
            &contents,
            &entries,
            table_at,
            image_at,
            self.image.len(),
            (self.drm_type, self.content_type, self.sku_flag),
            self.cache_size,
            gaps,
        )
    }

    /// Give every computed entry the size it will occupy.
    ///
    /// Sizes before contents, because the two digest tables are one slot per entry and the
    /// entry table cannot be laid out until every size is known.
    /// The passcode in force, which is the fake one unless a caller said otherwise.
    ///
    /// Named apart from the setter because a builder method and an accessor sharing a name is
    /// a compile error, and the setter is the one a caller sees.
    fn passcode_bytes(&self) -> &[u8] {
        self.passcode
            .as_deref()
            .unwrap_or(keys::FAKE_PASSCODE.as_slice())
    }

    fn size_computed_bodies(
        &self,
        contents: &mut [(u32, Vec<u8>)],
        count: usize,
    ) -> Result<(), WriteError> {
        for (id, body) in contents.iter_mut() {
            match *id {
                id if id == derive::entry::DIGESTS => {
                    *body = vec![0_u8; count.saturating_mul(DIGEST)];
                }
                id if id == derive::entry::TABLE_COPY => {
                    *body = vec![0_u8; count.saturating_mul(ENTRY_SIZE)];
                }
                id if id == derive::entry::MANIFEST => *body = vec![0_u8; MANIFEST_LEN],
                // Four bytes per 64 KiB of the finished package.
                id if id == derive::entry::PLAYGO_CHUNK_SHA => {
                    *body = vec![0_u8; playgo_len(&self.image)];
                }
                // Both licences come from the content id. `Licence::build` reproduces a real
                // one byte for byte, so these are computed rather than demanded.
                id if id == entry_id::LICENSE_DAT => {
                    *body = crate::licence::Licence::build(
                        &content_id_bytes(&self.content_id),
                        self.drm_type,
                        self.content_type,
                        self.sku_flag,
                    )
                    .map_err(|_| WriteError::LicenceFailed)?
                    .bytes;
                }
                id if id == entry_id::LICENSE_INFO => {
                    *body = crate::licence::Licence::info(&content_id_bytes(&self.content_id));
                }
                // The key blobs, which are what a console unwraps to reach the filesystem.
                // `0x10` is stored in the clear; `0x20` is encrypted below like the licences.
                id if id == entry_id::ENTRY_KEYS => {
                    *body = keys::entry_keys_blob(
                        &content_id_bytes(&self.content_id),
                        self.passcode_bytes(),
                    )
                    .map_err(|_| WriteError::LicenceFailed)?;
                }
                id if id == entry_id::IMAGE_KEY => {
                    *body = keys::image_key_blob(
                        &content_id_bytes(&self.content_id),
                        self.passcode_bytes(),
                    )
                    .map_err(|_| WriteError::LicenceFailed)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Write the header, the entry table and every entry body into the prepared buffer.
    #[allow(
        clippy::too_many_arguments,
        reason = "the header is one structure and threading it through a struct would only move \
                  the same fields somewhere else"
    )]
    fn emit(
        buffer: Vec<u8>,
        content_id: &str,
        contents: &[(u32, Vec<u8>)],
        entries: &[Entry],
        table_at: usize,
        image_at: usize,
        image_len: usize,
        kind: (u16, u16, u16),
        cache_size: Option<u32>,
        gaps: Vec<Gap>,
    ) -> Result<Built, WriteError> {
        let count = entries.len();
        let mut out = buffer;
        out.get_mut(..MAGIC.len())
            .ok_or(WriteError::TooLarge)?
            .copy_from_slice(&MAGIC);
        put32(
            &mut out,
            COUNT_AT,
            u32::try_from(count).map_err(|_| WriteError::TooLarge)?,
        );
        put32(
            &mut out,
            TABLE_AT,
            u32::try_from(table_at).map_err(|_| WriteError::TooLarge)?,
        );
        put64(&mut out, IMAGE_AT, image_at.try_into().unwrap_or(u64::MAX));

        write_header_fields(
            &mut out, count, entries, image_at, image_len, kind, cache_size,
        )?;

        let id = content_id.as_bytes();
        if let Some(slot) = out.get_mut(CONTENT_ID_AT..CONTENT_ID_AT.saturating_add(id.len())) {
            slot.copy_from_slice(id);
        }

        for (slot, entry) in entries.iter().enumerate() {
            let record = table_at.saturating_add(slot.saturating_mul(ENTRY_SIZE));
            put32(&mut out, record, entry.id);
            put32(&mut out, record.saturating_add(0x04), entry.name_offset);
            put32(&mut out, record.saturating_add(0x08), entry.flags1);
            put32(&mut out, record.saturating_add(0x0C), entry.flags2);
            put32(&mut out, record.saturating_add(0x10), entry.offset);
            put32(&mut out, record.saturating_add(0x14), entry.size);
        }
        for (id, body) in contents {
            if let Some(entry) = entries.iter().find(|e| e.id == *id) {
                let at = entry.offset as usize;
                if let Some(slot) = out.get_mut(at..at.saturating_add(body.len())) {
                    slot.copy_from_slice(body);
                }
            }
        }
        // Everything the bodies feed: the SC-entry hashes, the body and digest-table digests,
        // and the whole-header digest and signature. Only correct now that the bodies are in.
        finalize_digests(&mut out, entries)?;

        Ok(Built {
            bytes: out,
            image_at: image_at.try_into().unwrap_or(u64::MAX),
            entries: count,
            gaps,
        })
    }

    /// Entry `0x80`, filled as far as it is understood.
    fn manifest(&self, contents: &[(u32, Vec<u8>)], gaps: &mut Vec<Gap>) -> Vec<u8> {
        let mut out = vec![0_u8; MANIFEST_LEN];
        if let Some(slot) = out.get_mut(..derive::manifest::LEADING.len()) {
            slot.copy_from_slice(&derive::manifest::LEADING);
        }
        // The fixed word three packages agree on. Writing zero here is a difference from every
        // real package, and this crate was doing exactly that.
        if let Some(slot) =
            out.get_mut(derive::manifest::FIXED_1C..derive::manifest::FIXED_1C.saturating_add(4))
        {
            slot.copy_from_slice(&derive::manifest::FIXED_1C_VALUE.to_be_bytes());
        }
        // GameDigest (the image) and ParamDigest (the param.sfo) - the two this crate always had.
        put_digest(&mut out, derive::manifest::IMAGE_DIGEST, &self.image);
        if let Some((_, sfo)) = contents.iter().find(|(id, _)| *id == entry_id::PARAM_SFO) {
            put_digest(&mut out, derive::manifest::PARAM_SFO_DIGEST, sfo);

            // ContentDigest and MajorParamDigest, computed from the param.sfo per LibOrbisPkg.
            // These were left blank and reported as gaps; a console reads them as the content's
            // identity, so a package built without them describes nothing. HeaderDigest (`0x60`)
            // still cannot be taken here - it hashes header fields that do not exist until `emit`
            // writes them - so it stays a gap that `finalize_digests` fills.
            if let Ok(parsed) = selfish_title::sfo::Sfo::parse(sfo) {
                let major = major_param_string(&parsed);
                let major_digest: [u8; DIGEST] = Sha256::digest(major.as_bytes()).into();
                if let Some(slot) = out.get_mut(
                    derive::manifest::MAJOR_PARAM_DIGEST
                        ..derive::manifest::MAJOR_PARAM_DIGEST + DIGEST,
                ) {
                    slot.copy_from_slice(&major_digest);
                }
                let content_digest = self.content_digest(&major_digest);
                if let Some(slot) = out.get_mut(
                    derive::manifest::CONTENT_DIGEST..derive::manifest::CONTENT_DIGEST + DIGEST,
                ) {
                    slot.copy_from_slice(&content_digest);
                }
            } else {
                gaps.push(Gap {
                    entry: derive::entry::MANIFEST,
                    offset: derive::manifest::CONTENT_DIGEST,
                    length: DIGEST,
                    what: "the content digest, computed from param.sfo",
                });
                gaps.push(Gap {
                    entry: derive::entry::MANIFEST,
                    offset: derive::manifest::MAJOR_PARAM_DIGEST,
                    length: DIGEST,
                    what: "the major-param digest, computed from param.sfo",
                });
            }
        }
        // The one slot left: HeaderDigest, filled once the header is written.
        gaps.push(Gap {
            entry: derive::entry::MANIFEST,
            offset: derive::manifest::HEADER_DIGEST,
            length: DIGEST,
            what: "the header digest, filled by finalize_digests once the header exists",
        });
        gaps.sort_by_key(|gap| gap.offset);
        out
    }

    /// The content digest, `ComputeContentDigest` in `LibOrbisPkg`.
    ///
    /// SHA-256 of the content id, twelve zero bytes, the DRM and content types big-endian, the
    /// image digest (for game and additional-content types, which is what this crate builds),
    /// then the major-param digest.
    fn content_digest(&self, major_param_digest: &[u8; DIGEST]) -> [u8; DIGEST] {
        let mut hasher = Sha256::new();
        let mut id = [0_u8; 36];
        let bytes = self.content_id.as_bytes();
        // A content id longer than the field is truncated and a shorter one leaves zeroes, which
        // is what the fixed-width field means. Written through `get` so an over-long id cannot
        // panic here - the length is the caller's and this is a digest, not a validator.
        let take = bytes.len().min(id.len());
        if let (Some(into), Some(from)) = (id.get_mut(..take), bytes.get(..take)) {
            into.copy_from_slice(from);
        }
        hasher.update(id);
        hasher.update([0_u8; 12]);
        hasher.update(u32::from(self.drm_type).to_be_bytes());
        hasher.update(u32::from(self.content_type).to_be_bytes());
        hasher.update(Sha256::digest(&self.image));
        hasher.update(major_param_digest);
        hasher.finalize().into()
    }
}

/// What was produced, and how complete it is.
#[derive(Debug, Clone)]
pub struct Built {
    /// The package.
    pub bytes: Vec<u8>,
    /// Where the image was placed.
    pub image_at: u64,
    /// How many entries it carries.
    pub entries: usize,
    /// Every region this crate could not fill.
    ///
    /// **Empty means nothing was left blank**, not that the package is correct. A caller that
    /// ignores this ships a package with holes in it and finds out from a console.
    pub gaps: Vec<Gap>,
}

impl Built {
    /// Whether every byte this crate wrote is one it can account for.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.gaps.is_empty()
    }
}

/// A region left blank because nothing established says what goes in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    /// Which entry.
    pub entry: u32,
    /// Where in it.
    pub offset: usize,
    /// How many bytes.
    pub length: usize,
    /// What is known about it, which is usually only what it is not.
    pub what: &'static str,
}

/// How long the block-digest table will be for a package carrying this image.
///
/// Everything before the image is a fixed layout once the entry count is known, so this is
/// exact rather than an estimate - but it is computed rather than measured because the table's
/// own size is one of the things that decides where the image lands.
fn playgo_len(image: &[u8]) -> usize {
    // Everything before the image, which is now a fixed offset rather than something that
    // moves with the entry sizes. It used to be computed from the entries and rounded up, and
    // when the image moved to its real place that estimate silently went stale - the table came
    // out sized for a package half a megabyte shorter than the one being built.
    //
    // Caught by the test that re-runs the derivation against this crate's own output, which is
    // the whole reason that test exists.
    IMAGE_OFFSET
        .saturating_add(image.len())
        .checked_div(derive::PLAYGO_BLOCK)
        .unwrap_or(0)
        .saturating_mul(derive::PLAYGO_SLOT)
}

/// Encrypt the licence entries in place, before anything digests them.
///
/// Order matters: the digest table covers what a package **stores**, so a reader checking it
/// against the ciphertext would otherwise find the plaintext's digest recorded there.
fn encrypt_licences(
    contents: &mut [(u32, Vec<u8>)],
    entries: &[Entry],
    content_id: &str,
    passcode: &[u8],
) -> Result<(), WriteError> {
    for entry in entries {
        let index = match entry.id {
            entry_id::LICENSE_DAT => LICENCE_KEY_INDEX,
            entry_id::LICENSE_INFO => LICENCE_INFO_KEY_INDEX,
            entry_id::IMAGE_KEY => keys::IMAGE_KEY_INDEX,
            _ => continue,
        };
        let row = entry_row(entry);
        if let Some((_, body)) = contents.iter_mut().find(|(id, _)| *id == entry.id) {
            // The builder's passcode, not the fake one. Hardcoding the fake passcode here
            // encrypted these entries under a key the package's own key blobs do not lead to,
            // so a package built with any other passcode could not be opened - by a console or
            // by this crate's own reader. Caught by a test that keys a package differently.
            keys::encrypt_body(&row, &content_id_bytes(content_id), passcode, index, body)
                .map_err(|_| WriteError::LicenceFailed)?;
        }
    }
    Ok(())
}

/// The key index each licence entry declares, measured from every package examined.
const LICENCE_KEY_INDEX: u32 = 3;
/// The shorter record declares a different one.
const LICENCE_INFO_KEY_INDEX: u32 = 2;

fn entry_name(id: u32) -> Option<&'static str> {
    match id {
        0x1000 => Some("param.sfo"),
        0x1001 => Some("playgo-chunk.dat"),
        0x1002 => Some("playgo-chunk.sha"),
        0x1003 => Some("playgo-manifest.xml"),
        0x1004 => Some("pronunciation.xml"),
        0x1005 => Some("pronunciation.sig"),
        0x1006 => Some("pic1.png"),
        0x100b => Some("shareparam.json"),
        0x100d => Some("save_data.png"),
        0x1200 => Some("icon0.png"),
        0x1220 => Some("pic0.png"),
        0x1280 => Some("icon0.dds"),
        0x12a0 => Some("pic0.dds"),
        0x12c0 => Some("pic1.dds"),
        _ => None,
    }
}

fn resolve_name_offset(id: u32, contents: &[(u32, Vec<u8>)]) -> u32 {
    let Some(name) = entry_name(id) else {
        return 0;
    };
    let Some((_, names_body)) = contents.iter().find(|(held, _)| *held == 0x200) else {
        return 0;
    };
    let target = format!("\0{name}\0");
    if let Some(pos) = names_body
        .windows(target.len())
        .position(|w| w == target.as_bytes())
    {
        return u32::try_from(pos.saturating_add(1)).unwrap_or(0);
    }
    let target_prefix = format!("\0{name}");
    if let Some(pos) = names_body
        .windows(target_prefix.len())
        .position(|w| w == target_prefix.as_bytes())
    {
        return u32::try_from(pos.saturating_add(1)).unwrap_or(0);
    }
    0
}

/// One entry's table row, as it will be written.
///
/// The derivation hashes the row, so a writer has to produce the same thirty-two bytes the
/// reader will later find in the table - not the entry it came from.
fn entry_row(entry: &Entry) -> [u8; ENTRY_SIZE] {
    let mut row = [0_u8; ENTRY_SIZE];
    put32(&mut row, 0x00, entry.id);
    put32(&mut row, 0x04, entry.name_offset);
    put32(&mut row, 0x08, entry.flags1);
    put32(&mut row, 0x0C, entry.flags2);
    put32(&mut row, 0x10, entry.offset);
    put32(&mut row, 0x14, entry.size);
    row
}

/// The content id as the licence wants it: the bytes, NUL padded to its field.
fn content_id_bytes(id: &str) -> Vec<u8> {
    let mut out = vec![0_u8; CONTENT_ID_LEN];
    let take = id.len().min(CONTENT_ID_LEN);
    if let (Some(into), Some(from)) = (out.get_mut(..take), id.as_bytes().get(..take)) {
        into.copy_from_slice(from);
    }
    out
}

/// Length of the digest manifest, which is fixed in every sample.
const MANIFEST_LEN: usize = 0x180;
/// Length of the all-zero entry.
const ZEROS_LEN: usize = 0x2000;

fn set(contents: &mut [(u32, Vec<u8>)], id: u32, body: Vec<u8>) {
    if let Some((_, slot)) = contents.iter_mut().find(|(held, _)| *held == id) {
        *slot = body;
    }
}

fn put_digest(out: &mut [u8], at: usize, over: &[u8]) {
    let mut hasher = Sha256::new();
    hasher.update(over);
    let digest: [u8; DIGEST] = hasher.finalize().into();
    if let Some(slot) = out.get_mut(at..at.saturating_add(DIGEST)) {
        slot.copy_from_slice(&digest);
    }
}

/// The major-param string `LibOrbisPkg` hashes into the content and major-param digests.
///
/// `"ATTRIBUTE"` then its value, optionally `"ATTRIBUTE2"` and its value, then `"CATEGORY"`,
/// `"FORMAT"` and `"PUBTOOLVER"` the same way. Integer values render as `0x` and eight hex
/// digits, which is how the SFO value type stringifies; text values render as themselves.
fn major_param_string(sfo: &selfish_title::sfo::Sfo) -> String {
    use selfish_title::sfo::Value;
    fn render(value: &Value) -> String {
        match value {
            Value::Text(text) | Value::TextUnterminated(text) => text.clone(),
            Value::Integer(number) => format!("0x{number:08x}"),
            Value::Unknown(..) => String::new(),
        }
    }
    let mut out = String::new();
    for key in [
        "ATTRIBUTE",
        "ATTRIBUTE2",
        "CATEGORY",
        "FORMAT",
        "PUBTOOLVER",
    ] {
        if let Some(value) = sfo.get(key) {
            out.push_str(key);
            out.push_str(&render(value));
        }
    }
    out
}

/// Write everything in the header past the four fields this crate already knew.
///
/// Split out of `emit` because it is one self-contained structure, and because a console
/// reading any of it as zero is a package that does not install. Every offset and every
/// constant here was measured out of a real package and cross-checked against `LibOrbisPkg@6434772`'s
/// writer. (D056)
fn write_header_fields(
    out: &mut [u8],
    count: usize,
    entries: &[Entry],
    image_at: usize,
    image_len: usize,
    kind: (u16, u16, u16),
    cache_size: Option<u32>,
) -> Result<(), WriteError> {
    // The rest of the header. Everything here was zero until it was measured out of a real
    // package, and a console reading `IMAGE_SIZE` as zero has nothing to mount. (D056)
    let (drm_type, content_type, _sku) = kind;
    let image_len64 = u64::try_from(image_len).map_err(|_| WriteError::TooLarge)?;
    let image_at64 = u64::try_from(image_at).map_err(|_| WriteError::TooLarge)?;
    let total = image_at64
        .checked_add(image_len64)
        .ok_or(WriteError::TooLarge)?;

    put32(out, header::FLAGS, header_value::FLAGS);
    put32(out, header::UNK_0C, header_value::UNK_0C);
    // **Not the entry count.** This crate wrote the total here, and three real packages hold
    // `6` regardless of how many entries they carry (14, 23, 14). It counts the *SC* entries -
    // the format's own, ahead of the title's - and a console reads it to find them. Writing the
    // total made an installer read past them and reject the package. (measured 3/3)
    put16(out, header::SC_ENTRY_COUNT, header_value::SC_ENTRY_COUNT);
    put16(
        out,
        header::ENTRY_COUNT_2,
        u16::try_from(count).unwrap_or(u16::MAX),
    );
    // The size of the *entry data*, not the distance to the image.
    //
    // Written as "everything between the table and the image", which is the whole gap and was
    // two orders out: real packages hold 3584 and 4160 where that arithmetic gives ~520000.
    // Measured, it is the summed size of the SC entries rounded down to a 512-byte boundary
    // (3659 -> 3584, 4347 -> 4160), and the padding after them is not counted.
    let sc_data: usize = entries
        .iter()
        .take(usize::from(header_value::SC_ENTRY_COUNT))
        .map(|entry| usize::try_from(entry.size).unwrap_or(0))
        .sum();
    put32(
        out,
        header::MAIN_ENTRY_DATA_SIZE,
        u32::try_from(sc_data & !0x1FF).unwrap_or(0),
    );
    put64(out, header::BODY_OFFSET, header_value::BODY_OFFSET);
    // **Derived, not constant.** This was `0x7E000`, on the strength of two packages holding it -
    // and the third holds `0x57E000`. The rule all three fit is that the body runs from
    // `BODY_OFFSET` to where the image begins:
    //
    //     body_size = image_offset - body_offset
    //
    //     item   524288 - 8192 =  516096   (0x7E000)
    //     lapy  5767168 - 8192 = 5758976   (0x57E000)
    //     store  524288 - 8192 =  516096   (0x7E000)
    //
    // The constant was right for every package this crate had built only because they all put the
    // image at `0x80000`. It would have been silently wrong for any that did not - and
    // `BODY_DIGEST` hashes exactly this region, so the digest would have covered the wrong bytes
    // while looking perfectly well-formed. Found by auditing the other constants after D070.
    let body_size = image_at
        .checked_sub(usize::try_from(header_value::BODY_OFFSET).unwrap_or(0))
        .and_then(|len| u64::try_from(len).ok())
        .unwrap_or(header_value::BODY_SIZE);
    put64(out, header::BODY_SIZE, body_size);
    put32(out, header::DRM_TYPE, u32::from(drm_type));
    put32(out, header::CONTENT_TYPE, u32::from(content_type));
    put32(out, header::CONTENT_FLAGS, header_value::CONTENT_FLAGS);
    // What the installer promotes, which is everything ahead of the image.
    //
    // Zero here is what this crate wrote, and it is the field an installer reads to decide there
    // is nothing to promote. In all three real packages it equals the image offset exactly
    // (0x80000, 0x580000, 0x80000). (measured 3/3)
    put32(
        out,
        header::PROMOTE_SIZE,
        u32::try_from(image_at).unwrap_or(0),
    );
    put32(out, header::VERSION_DATE, header_value::VERSION_DATE);
    put32(out, header::VERSION_HASH, header_value::VERSION_HASH);
    put32(out, header::EKC_VERSION, header_value::EKC_VERSION);

    put32(out, header::UNK_400, header_value::UNK_400);
    put32(out, header::IMAGE_COUNT, header_value::IMAGE_COUNT);
    put64(out, header::PFS_FLAGS, header_value::PFS_FLAGS);
    put64(out, header::IMAGE_SIZE, image_len64);
    put64(out, header::MOUNT_IMAGE_OFFSET, 0);
    put64(out, header::MOUNT_IMAGE_SIZE, total);
    put64(out, header::PACKAGE_SIZE, total);
    put32(out, header::SIGNED_SIZE, header_value::SIGNED_SIZE);
    put32(
        out,
        header::CACHE_SIZE,
        cache_size.unwrap_or(header_value::CACHE_SIZE),
    );

    // Two digests over the image, taken now because the image is already in the buffer and
    // nothing written after this point lands inside it.
    let signed_len = usize::try_from(header_value::SIGNED_SIZE).unwrap_or(0);
    if let Some(region) = out.get(image_at..image_at.saturating_add(signed_len)) {
        let digest: [u8; 32] = Sha256::digest(region).into();
        if let Some(slot) = out.get_mut(header::SIGNED_DIGEST..header::SIGNED_DIGEST + 32) {
            slot.copy_from_slice(&digest);
        }
    }
    if let Some(region) = out.get(image_at..image_at.saturating_add(image_len)) {
        let digest: [u8; 32] = Sha256::digest(region).into();
        if let Some(slot) = out.get_mut(header::IMAGE_DIGEST..header::IMAGE_DIGEST + 32) {
            slot.copy_from_slice(&digest);
        }
    }
    // The digests that cover the entry data (`0x100`-`0x17F`) and the whole-header digest and
    // signature (`0xFE0`, `0x1000`) are **not** written here. They are taken over the finished
    // buffer, once the entry bodies are in place, by `finalize_digests` at the end of `emit`.
    // Taking them here would hash a body region that is still zero - the mistake this note
    // replaces.
    Ok(())
}

/// The digests and signature that cover the whole assembled package.
///
/// Called once the header, entry table, entry bodies and image are all in `out`, because every
/// digest here reads bytes that are only correct by then. The order follows `LibOrbisPkg`'s
/// `CalcBodyDigests` then the final header digest and signature (`PkgBuilder.cs`), each recipe
/// confirmed against a real package before it was written:
///
/// - `0x100` `sc_entries1` = SHA-256 of five SC entry bodies, in the order `0x10,0x20,0x80,0x100,0x1`
/// - `0x120` `sc_entries2` = the same minus `0x1`, with `0x100` truncated to `sc_entry_count * 0x20`
/// - `0x140` `digest_table_hash` = SHA-256 of entry `0x1`
/// - `0x160` `body_digest` = SHA-256 of the body region
/// - `0xFE0` header digest = SHA-256 of `out[0..0xFE0]`
/// - `0x1000` signature = the header digest hash wrapped under pkg public key 3 (D054's primitive)
///
/// The signature is a wrap under a **published** key, exactly as the key blobs are; it asserts
/// nothing about the vendor, and a console with the matching public keyset accepts it where it
/// accepts fake packages at all. (principle 6)
// Long for the same reason `build` is: these digests have to be written in this order, because
// each covers bytes an earlier one put there. The header digest covers the region the four before
// it wrote into, and the signature covers the header digest. Splitting it into named halves would
// let a caller run them out of order, which is the one mistake the ordering exists to prevent.
#[allow(clippy::too_many_lines)]
fn finalize_digests(out: &mut [u8], entries: &[Entry]) -> Result<(), WriteError> {
    let body_of = |id: u32| -> Option<(usize, usize)> {
        entries.iter().find(|entry| entry.id == id).map(|entry| {
            (
                usize::try_from(entry.offset).unwrap_or(0),
                usize::try_from(entry.size).unwrap_or(0),
            )
        })
    };
    let slice = |out: &[u8], id: u32, cap: Option<usize>| -> Vec<u8> {
        body_of(id).map_or_else(Vec::new, |(at, len)| {
            let len = cap.map_or(len, |c| len.min(c));
            out.get(at..at.saturating_add(len))
                .unwrap_or_default()
                .to_vec()
        })
    };

    // HeaderDigest (`0x60`) in the manifest: SHA-256 of the top of the header and its image
    // block, which only exist now. This changes the manifest body, so the digest table below is
    // recomputed after it rather than trusting the one built before the header existed.
    if let Some((man_at, _)) = body_of(derive::entry::MANIFEST) {
        let mut header_slice = Vec::with_capacity(64 + 128);
        header_slice.extend_from_slice(out.get(..64).unwrap_or_default());
        header_slice.extend_from_slice(out.get(0x400..0x480).unwrap_or_default());
        let digest: [u8; 32] = Sha256::digest(&header_slice).into();
        let at = man_at.saturating_add(derive::manifest::HEADER_DIGEST);
        if let Some(slot) = out.get_mut(at..at.saturating_add(32)) {
            slot.copy_from_slice(&digest);
        }
    }

    // Recompute the digest table (`0x1`): one SHA-256 per entry body, its own slot zeroed. The
    // manifest just changed, so its slot here would otherwise be stale.
    if let Some((dt_at, dt_len)) = body_of(derive::entry::DIGESTS) {
        let mut table = vec![0_u8; dt_len];
        for (slot, entry) in entries.iter().enumerate() {
            let at = slot.saturating_mul(DIGEST);
            if entry.id == derive::entry::DIGESTS {
                continue; // self-slot stays zero
            }
            let digest: [u8; 32] = Sha256::digest(slice(out, entry.id, None)).into();
            if let Some(dst) = table.get_mut(at..at.saturating_add(DIGEST)) {
                dst.copy_from_slice(&digest);
            }
        }
        if let Some(dst) = out.get_mut(dt_at..dt_at.saturating_add(table.len())) {
            dst.copy_from_slice(&table);
        }
    }

    // sc_entries1 and sc_entries2. The second truncates the entry-table copy (`0x100`) to the
    // SC entry count times a record, which is the one difference between them.
    let sc_count = usize::from(header_value::SC_ENTRY_COUNT).saturating_mul(0x20);
    let mut sc1 = Sha256::new();
    for id in [
        0x10_u32,
        0x20,
        derive::entry::MANIFEST,
        0x100,
        derive::entry::DIGESTS,
    ] {
        sc1.update(slice(out, id, None));
    }
    let sc1: [u8; 32] = sc1.finalize().into();
    let mut sc2 = Sha256::new();
    for id in [0x10_u32, 0x20, derive::entry::MANIFEST] {
        sc2.update(slice(out, id, None));
    }
    sc2.update(slice(out, 0x100, Some(sc_count)));
    let sc2: [u8; 32] = sc2.finalize().into();
    if let Some(s) = out.get_mut(header::SC_ENTRIES1_HASH..header::SC_ENTRIES1_HASH + 32) {
        s.copy_from_slice(&sc1);
    }
    if let Some(s) = out.get_mut(header::SC_ENTRIES2_HASH..header::SC_ENTRIES2_HASH + 32) {
        s.copy_from_slice(&sc2);
    }

    // digest_table_hash: the digest table entry, hashed whole.
    let dt: [u8; 32] = Sha256::digest(slice(out, derive::entry::DIGESTS, None)).into();
    if let Some(s) = out.get_mut(header::DIGEST_TABLE_DIGEST..header::DIGEST_TABLE_DIGEST + 32) {
        s.copy_from_slice(&dt);
    }

    // body_digest: the whole body region.
    //
    // The length is read back out of the header rather than taken from the constant, so it cannot
    // disagree with what the header declares. Writing `BODY_SIZE` as a derived value and hashing a
    // constant-sized region would be two answers to one question, and the digest is the half that
    // fails silently: it would be well-formed and cover the wrong bytes. (D070)
    let body_at = usize::try_from(header_value::BODY_OFFSET).unwrap_or(0);
    let body_len = out
        .get(header::BODY_SIZE..header::BODY_SIZE.saturating_add(8))
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map_or(0, |bytes| {
            usize::try_from(u64::from_be_bytes(bytes)).unwrap_or(0)
        });
    if let Some(region) = out.get(body_at..body_at.saturating_add(body_len)) {
        let digest: [u8; 32] = Sha256::digest(region).into();
        if let Some(s) = out.get_mut(header::BODY_DIGEST..header::BODY_DIGEST + 32) {
            s.copy_from_slice(&digest);
        }
    }

    // The whole-header digest, then the signature over the header including that digest.
    if let Some(region) = out.get(..0xFE0) {
        let digest: [u8; 32] = Sha256::digest(region).into();
        if let Some(s) = out.get_mut(0xFE0..0xFE0 + 32) {
            s.copy_from_slice(&digest);
        }
    }
    if let Some(region) = out.get(..0x1000) {
        let digest: [u8; 32] = Sha256::digest(region).into();
        let modulus = keys::pkg_public_modulus(3).ok_or(WriteError::KeysUnreadable)?;
        let sig =
            crate::wrap::wrap_key(&modulus, &digest).map_err(|_| WriteError::KeysUnreadable)?;
        if let Some(s) = 0x1000_usize
            .checked_add(sig.len())
            .and_then(|end| out.get_mut(0x1000..end))
        {
            s.copy_from_slice(&sig);
        }
    }
    Ok(())
}

/// Where an entry falls in a real package's body layout.
///
/// **Not** ascending id. A real package lays the format's own entries first - keys, image key,
/// general digests, the metas table, the digest table - then the entry names, the playgo trio,
/// the licences, `param.sfo`, the reserved block and the icon. The first four are exactly
/// `main_ent_data_size` long, which is what puts the metas entry (the table) at `0x2A80`. An id
/// this does not list sorts to the end, so an unexpected entry never displaces a known one.
fn layout_rank(id: u32) -> usize {
    const ORDER: [u32; 14] = [
        0x10, 0x20, 0x80, 0x100, 0x1, 0x200, 0x1001, 0x1002, 0x1003, 0x400, 0x401, 0x1000, 0x409,
        0x1200,
    ];
    ORDER.iter().position(|&x| x == id).unwrap_or(ORDER.len())
}

fn put16(out: &mut [u8], at: usize, value: u16) {
    if let Some(slot) = out.get_mut(at..at.saturating_add(2)) {
        slot.copy_from_slice(&value.to_be_bytes());
    }
}

fn put32(out: &mut [u8], at: usize, value: u32) {
    if let Some(slot) = out.get_mut(at..at.saturating_add(4)) {
        slot.copy_from_slice(&value.to_be_bytes());
    }
}

fn put64(out: &mut [u8], at: usize, value: u64) {
    if let Some(slot) = out.get_mut(at..at.saturating_add(8)) {
        slot.copy_from_slice(&value.to_be_bytes());
    }
}

/// Why a package could not be assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WriteError {
    /// Entries with no established meaning that nobody supplied.
    ///
    /// Named rather than counted: the fix is always to go and find them.
    Missing(Vec<u32>),
    /// An entry this crate computes was also handed in.
    AlreadyComputed(u32),
    /// The content id does not fit its field.
    ContentIdTooLong(usize),
    /// Something exceeded what a 32-bit field can address.
    TooLarge,
    /// The licence could not be built, which means the keyset could not be read.
    LicenceFailed,
    /// The header signature could not be produced, which means the keyset could not be read.
    KeysUnreadable,
}

impl core::fmt::Display for WriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Missing(ids) => {
                write!(
                    f,
                    "no contents for entr{}:",
                    if ids.len() == 1 { "y" } else { "ies" }
                )?;
                for id in ids {
                    write!(f, " {id:#x}")?;
                }
                write!(
                    f,
                    " - nothing here can compute them, so they must be supplied"
                )
            }
            Self::AlreadyComputed(id) => write!(
                f,
                "entry {id:#x} is computed from the others and must not also be supplied"
            ),
            Self::ContentIdTooLong(len) => {
                write!(
                    f,
                    "the content id is {len} bytes and the field holds {CONTENT_ID_LEN}"
                )
            }
            Self::TooLarge => write!(f, "a field cannot address something this large"),
            Self::LicenceFailed => write!(f, "the licence could not be built"),
            Self::KeysUnreadable => write!(f, "the header signature keyset could not be read"),
        }
    }
}

impl std::error::Error for WriteError {}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use sha2::{Digest as _, Sha256};

    use super::{Builder, WriteError};
    use crate::{Package, derive, entry_id};

    /// Everything a package needs that this crate cannot compute.
    fn supplied(builder: Builder) -> Builder {
        let mut builder = builder;
        for id in entry_id::ALWAYS_PRESENT {
            if id == entry_id::PARAM_SFO_ZEROS || super::COMPUTED.contains(&id) {
                continue;
            }
            // Distinct contents per entry, so a digest table that mixed two up would fail.
            builder = builder.entry(id, vec![u8::try_from(id & 0xFF).unwrap_or(0); 64]);
        }
        builder
    }

    /// The body runs from its offset to the image, and is not the constant it used to be.
    ///
    /// `BODY_SIZE` was `0x7E000`, which two of three real packages hold and the third
    /// (`0x57E000`) does not. It is `image_offset - body_offset`, and it had been right in every
    /// package this crate built only because they all put the image at `0x80000`. `BODY_DIGEST`
    /// hashes exactly this region, so the failure it would have caused is a well-formed digest
    /// over the wrong bytes - which nothing else here would have noticed. (D070)
    #[test]
    fn the_body_size_is_derived_from_where_the_image_begins() {
        let built = supplied(Builder::new().content_id("UP0000-TEST00001_00-0000000000000000"))
            .image(vec![0xAB; 0x2000])
            .build()
            .expect("a package");

        let read = |at: usize| -> u64 {
            let bytes: [u8; 8] = built.bytes[at..at + 8].try_into().expect("eight bytes");
            u64::from_be_bytes(bytes)
        };
        let body_offset = read(super::header::BODY_OFFSET);
        let body_size = read(super::header::BODY_SIZE);
        let image_at = built.image_at;

        assert_eq!(
            body_size,
            image_at - body_offset,
            "the body should run from its offset to the image"
        );
    }

    /// A package may not declare a cache larger than the filesystem being cached.
    ///
    /// A console refuses one that does - `Failed to enable GDDR5 cache`, `EINVAL` - after the
    /// outer image has already mounted, which is late enough to look like something else
    /// entirely. The default stays what every real package carries; this is the override that
    /// lets a small title say so. (D070)
    #[test]
    fn the_cache_size_can_be_clamped_below_the_default() {
        let small = 0xB_0000_u32;
        assert!(small < super::DEFAULT_CACHE_SIZE, "the point of the test");

        let built = supplied(Builder::new().content_id("UP0000-TEST00001_00-0000000000000000"))
            .image(vec![0xAB; 0x2000])
            .cache_size(small)
            .build()
            .expect("a package");

        let bytes: [u8; 4] = built.bytes[super::header::CACHE_SIZE..super::header::CACHE_SIZE + 4]
            .try_into()
            .expect("four bytes");
        assert_eq!(u32::from_be_bytes(bytes), small);

        // And a package that says nothing still gets the measured default.
        let default = supplied(Builder::new().content_id("UP0000-TEST00001_00-0000000000000000"))
            .image(vec![0xAB; 0x2000])
            .build()
            .expect("a package");
        let bytes: [u8; 4] = default.bytes
            [super::header::CACHE_SIZE..super::header::CACHE_SIZE + 4]
            .try_into()
            .expect("four bytes");
        assert_eq!(u32::from_be_bytes(bytes), super::DEFAULT_CACHE_SIZE);
    }

    #[test]
    fn a_package_this_crate_builds_is_one_it_can_read_back() {
        let built = supplied(Builder::new().content_id("UP0000-TEST00001_00-0000000000000000"))
            .image(vec![0xAB; 0x2000])
            .build()
            .expect("a package");

        let package = Package::parse(&built.bytes).expect("a readable package");
        assert_eq!(package.entries().len(), built.entries);
        assert_eq!(package.image_offset().expect("an offset"), built.image_at);
        assert!(package.missing_expected_entries().is_empty());
    }

    #[test]
    fn the_derivation_holds_on_a_package_this_crate_built() {
        // The strongest check available: the same command that re-derives the entry meanings
        // from somebody else's packages, run against one of ours. If the writer and the
        // derivation disagree, one of them is wrong and this says so.
        let built = supplied(Builder::new().content_id("UP0000-TEST00001_00-0000000000000000"))
            .image(vec![0xCD; 0x1000])
            .build()
            .expect("a package");

        let package = Package::parse(&built.bytes).expect("a readable package");
        let derivation = derive::run(std::slice::from_ref(&package));
        assert!(
            derivation.is_consistent(),
            "the writer disagrees with the derivation: {:?}",
            derivation.findings
        );
    }

    #[test]
    fn the_image_digest_covers_the_image_that_was_supplied() {
        let image = vec![0x5A_u8; 0x3000];
        let built = supplied(Builder::new())
            .image(image.clone())
            .build()
            .expect("a package");

        let package = Package::parse(&built.bytes).expect("readable");
        let manifest = package
            .entry(derive::entry::MANIFEST)
            .and_then(|entry| package.entry_bytes(entry))
            .expect("a manifest");
        let at = derive::manifest::IMAGE_DIGEST;

        let mut hasher = Sha256::new();
        hasher.update(&image);
        let want: [u8; 32] = hasher.finalize().into();
        assert_eq!(&manifest[at..at + 32], &want);
    }

    #[test]
    fn the_licence_in_a_built_package_decrypts_and_verifies() {
        // The whole chain in one test: build a licence, encrypt it, flag the entry, lay out a
        // package, then read it back the way anything else would and check the signature. A
        // writer that gets the flags, the key index, the row or the derivation wrong fails
        // here rather than on a console.
        let id = "UP0000-TEST00001_00-0000000000000000";
        let built = supplied(Builder::new().content_id(id))
            .image(vec![0x11; 0x1000])
            .build()
            .expect("a package");

        let package = Package::parse(&built.bytes).expect("readable");
        let entry = package
            .entry(entry_id::LICENSE_DAT)
            .expect("a licence entry");
        assert!(
            entry.is_encrypted(),
            "the entry must declare itself encrypted"
        );

        let plain = crate::keys::decrypt_entry(&package, entry).expect("decrypts");
        assert_eq!(&plain[..4], b"RIF\0");
        let licence = crate::licence::Licence { bytes: plain };
        assert!(
            licence.signature_is_valid().expect("a keyset"),
            "the signature must verify against the keyset that produced it"
        );

        // And the shorter record names the same title.
        let info = package
            .entry(entry_id::LICENSE_INFO)
            .and_then(|e| crate::keys::decrypt_entry(&package, e).ok())
            .expect("an info record");
        assert_eq!(&info[..id.len()], id.as_bytes());
    }

    #[test]
    fn a_missing_entry_is_named_rather_than_zero_filled() {
        // The whole point of the module. Zero-filling would produce a package a console reads
        // and acts on, and the failure would surface as a refused install with no clue.
        let error = Builder::new().image(vec![0; 16]).build().unwrap_err();
        let WriteError::Missing(missing) = &error else {
            panic!("expected a missing-entry error, got {error:?}");
        };
        // What is left is the title's own content, which no format library can invent.
        assert!(missing.contains(&entry_id::PARAM_SFO), "param.sfo");
        for computed in [0x1_u32, 0x80, 0x100, 0x400, 0x401, 0x1002] {
            assert!(
                !missing.contains(&computed),
                "{computed:#x} is computed now, not demanded"
            );
        }
        assert!(error.to_string().contains("0x1000"));
    }

    #[test]
    fn a_computed_entry_cannot_also_be_supplied() {
        // Two sources for one entry is how a digest table stops matching what it describes.
        let error = supplied(Builder::new())
            .entry(derive::entry::DIGESTS, vec![0; 32])
            .build()
            .unwrap_err();
        assert_eq!(error, WriteError::AlreadyComputed(derive::entry::DIGESTS));
    }

    #[test]
    fn the_gaps_are_reported_rather_than_left_for_a_console_to_find() {
        let built = supplied(Builder::new())
            .image(vec![0; 16])
            .build()
            .expect("a package");
        assert!(!built.is_complete());
        assert_eq!(built.gaps.len(), 3, "three unfilled manifest slots");
        assert!(
            built
                .gaps
                .iter()
                .all(|gap| gap.entry == derive::entry::MANIFEST)
        );
    }
}
