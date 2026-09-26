//! Assembling a package.
//!
//! The builder writes the header and entry table and computes the digest table, the table
//! copy, the manifest digests, the playgo block digests, the licences, the key blobs, the
//! zero-filled `0x409` entry, the entry name table and the playgo chunk descriptor. The caller
//! supplies the image and the title's own content, such as `param.sfo` and the icon; a missing
//! required entry fails the build and names every one. Manifest slots the builder leaves zero
//! are reported as [`Built::gaps`]. Licences are signed with the debug RIF
//! keyset and declare themselves debug licences (D047).

use selfish_bytes::{write_be, write_slice};
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

/// Offsets of the remaining header fields.
///
/// Measured from a real package and matching `LibOrbisPkg@6434772`'s writer offset for offset
/// (D056).
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
    /// How long the image is. Zero here leaves nothing to mount.
    pub(super) const IMAGE_SIZE: usize = 0x418;
    /// Where the mount image begins.
    pub(super) const MOUNT_IMAGE_OFFSET: usize = 0x420;
    /// How long the mount image is.
    pub(super) const MOUNT_IMAGE_SIZE: usize = 0x428;
    /// The whole package's length, which is the file's length.
    pub(super) const PACKAGE_SIZE: usize = 0x430;
    /// How much of the image is signed.
    pub(super) const SIGNED_SIZE: usize = 0x438;
    /// How much of it the hardware caches.
    pub(super) const CACHE_SIZE: usize = 0x43C;
    /// A digest of the whole image.
    pub(super) const IMAGE_DIGEST: usize = 0x440;
    /// A digest of the image's first signed region.
    pub(super) const SIGNED_DIGEST: usize = 0x460;
    /// SHA-256 of the five SC entry bodies (`0x10,0x20,0x80,0x100,0x1`), measured in one package.
    pub(super) const SC_ENTRIES1_HASH: usize = 0x100;
    /// SHA-256 of four SC entry bodies, `0x100` truncated to `sc_entry_count * 0x20`, measured
    /// in one package.
    pub(super) const SC_ENTRIES2_HASH: usize = 0x120;
    /// SHA-256 of the digest table, entry `0x1`. The hardware refuses a package with this and
    /// its neighbouring slots zero with `0x80f00101`.
    pub(super) const DIGEST_TABLE_DIGEST: usize = 0x140;
    /// SHA-256 of the body, the region [`BODY_OFFSET`]..[`BODY_SIZE`] describes:
    /// `file[0x2000..0x2000 + 0x7E000]`.
    ///
    /// [`BODY_OFFSET`]: self::BODY_OFFSET
    /// [`BODY_SIZE`]: self::BODY_SIZE
    pub(super) const BODY_DIGEST: usize = 0x160;
}

/// The cache size every real package declares, and the default this crate writes.
///
/// Public so a caller can compare its inner image against it: a package whose inner filesystem
/// is smaller than the declared cache size does not mount. See [`Builder::cache_size`].
pub const DEFAULT_CACHE_SIZE: u32 = 0xD_0000;

/// Constants the header carries that are the same in every package examined.
mod header_value {
    /// The only flag a fake package sets.
    pub(super) const FLAGS: u32 = 0x01;
    /// Content flags. Zero reads as an unconfigured package; this is the value a working
    /// homebrew package carries.
    pub(super) const CONTENT_FLAGS: u32 = 0x0A00_0000;
    /// How many SC entries a package declares at `0x14`. Six in every package examined,
    /// whatever the total entry count, so it counts the format's own entries.
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
    /// How much the hardware caches.
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
/// Fixed at `0x80000`, as in real packages and `LibOrbisPkg@6434772`.
const IMAGE_OFFSET: usize = 0x80000;
/// Where the entry table begins: `0x2A80` in every package examined, whatever the entry count.
///
/// A fixed constant of the format. `scePlayGoCoreGetRawContentInfo` refuses a package with any
/// other table offset with `0x80f00101`.
const HEADER_RESERVED: usize = 0x2A80;

/// Entries this module fills in by itself.
///
/// A caller supplying one of these is refused rather than overridden, so the digest table
/// always matches the entries it describes.
const COMPUTED: [u32; 8] = [
    derive::entry::DIGESTS,
    derive::entry::TABLE_COPY,
    derive::entry::MANIFEST,
    derive::entry::PLAYGO_CHUNK_SHA,
    entry_id::LICENSE_DAT,
    entry_id::LICENSE_INFO,
    // The key blobs, computed from the passcode and the public keys (D054).
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
    /// The installer does not register a package with `content_type = 0`, so the defaults are
    /// a homebrew application's: DRM type `0x0F` (free/fake) and content type `0x1A`
    /// (`CONTENT_TYPE_GD`). Additional content or a patch overrides them with [`kind`].
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
    /// Defaults to [`keys::FAKE_PASSCODE`]. The filesystem key, both key blobs and the entry
    /// encryption all derive from it, so the image must be built with the same passcode.
    #[must_use]
    pub fn passcode(mut self, passcode: &[u8]) -> Self {
        self.passcode = Some(passcode.to_vec());
        self
    }

    /// The filesystem image, already built and encrypted.
    ///
    /// `selfish_pfs::write`, `selfish_pfs::pfsc` and `selfish_pfs::outer` produce one from a
    /// tree of files.
    #[must_use]
    pub fn image(mut self, image: Vec<u8>) -> Self {
        self.image = image;
        self
    }

    /// How much of the image the hardware may cache, overriding [`DEFAULT_CACHE_SIZE`].
    ///
    /// The hardware refuses an image whose inner filesystem is smaller than the declared cache
    /// size, with `sceFsMountGamePkg ***ERR*** Failed to enable GDDR5 cache` and `EINVAL`. The
    /// value is a ceiling; its meaning beyond that is unknown.
    #[must_use]
    pub fn cache_size(mut self, bytes: u32) -> Self {
        self.cache_size = Some(bytes);
        self
    }

    /// What the title is, for the licence.
    ///
    /// Overrides the application defaults [`new`] sets, for a patch or add-on.
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
    /// entry is missing. The missing-entry error names every one.
    pub fn build(self) -> Result<Built, WriteError> {
        if self.content_id.len() > CONTENT_ID_LEN {
            return Err(WriteError::ContentIdTooLong(self.content_id.len()));
        }
        if let Some((id, _)) = self.supplied.iter().find(|(id, _)| COMPUTED.contains(id)) {
            return Err(WriteError::AlreadyComputed(*id));
        }

        let mut contents = self.contents()?;
        let count = contents.len();
        self.size_computed_bodies(&mut contents, count)?;
        let (entries, table_at) = lay_out(&mut contents)?;
        encrypt_licences(
            &mut contents,
            &entries,
            &self.content_id,
            self.passcode_bytes(),
        )?;

        let mut buffer = vec![0_u8; IMAGE_OFFSET];
        buffer.extend_from_slice(&self.image);
        let gaps = self.fill_derived(&mut contents, &entries, &buffer);
        self.emit(buffer, &contents, &entries, table_at, gaps)
    }

    /// Every entry the package carries, in ascending id, with the ones this crate derives from
    /// the others added when the caller did not supply them.
    ///
    /// Entry `0x200` is the NUL-separated names of the entries present. Entry `0x1001` is fixed
    /// for a single-chunk title except for the package and inner filesystem sizes, both known
    /// here (D099). A supplied one of either wins, so a package can be rebuilt byte for byte.
    fn contents(&self) -> Result<Vec<(u32, Vec<u8>)>, WriteError> {
        let mut contents: Vec<(u32, Vec<u8>)> = self.supplied.clone();
        contents.push((entry_id::PARAM_SFO_ZEROS, vec![0_u8; ZEROS_LEN]));
        for id in COMPUTED {
            contents.push((id, Vec::new()));
        }
        contents.sort_by_key(|(id, _)| *id);
        contents.dedup_by_key(|(id, _)| *id);

        if !contents.iter().any(|(id, _)| *id == 0x200) {
            let ids: Vec<u32> = contents.iter().map(|(id, _)| *id).collect();
            contents.push((0x200, entry_names_table(&ids)));
            contents.sort_by_key(|(id, _)| *id);
        }

        let has_playgo = contents
            .iter()
            .any(|(id, _)| *id == derive::entry::PLAYGO_CHUNK_DAT);
        let inner_size = if has_playgo {
            None
        } else {
            inner_image_size(&self.image, &self.content_id, self.passcode_bytes())
        };
        if let Some(inner) = inner_size {
            let package_size = u64::try_from(IMAGE_OFFSET.saturating_add(self.image.len()))
                .map_err(|_| WriteError::TooLarge)?;
            let body = crate::playgo::chunk_dat(&self.content_id, package_size, inner);
            contents.push((derive::entry::PLAYGO_CHUNK_DAT, body));
            contents.sort_by_key(|(id, _)| *id);
        }

        let missing: Vec<u32> = entry_id::ALWAYS_PRESENT
            .iter()
            .copied()
            .filter(|id| !contents.iter().any(|(held, _)| held == id))
            .collect();
        if !missing.is_empty() {
            return Err(WriteError::Missing(missing));
        }
        Ok(contents)
    }

    /// Fill the entries computed from the others: the playgo block digests, the entry table
    /// copy, the manifest and the digest table, in that order, since each reads the ones
    /// before. Returns what the manifest could not fill.
    fn fill_derived(
        &self,
        contents: &mut [(u32, Vec<u8>)],
        entries: &[Entry],
        buffer: &[u8],
    ) -> Vec<Gap> {
        let playgo = derive::playgo_chunk_sha(buffer, IMAGE_OFFSET);
        set(contents, derive::entry::PLAYGO_CHUNK_SHA, playgo);
        set(
            contents,
            derive::entry::TABLE_COPY,
            derive::entry_table_copy(entries),
        );
        let mut gaps = Vec::new();
        let manifest = self.manifest(contents, &mut gaps);
        set(contents, derive::entry::MANIFEST, manifest);

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
        set(contents, derive::entry::DIGESTS, digests);
        gaps
    }

    /// The passcode in force, which is the fake one unless a caller said otherwise.
    fn passcode_bytes(&self) -> &[u8] {
        self.passcode
            .as_deref()
            .unwrap_or(keys::FAKE_PASSCODE.as_slice())
    }

    /// Give every computed entry the size it will occupy, and the contents of those that
    /// depend only on the content id and passcode.
    ///
    /// Sizes come first because the digest tables are one slot per entry, and the entry table
    /// cannot be laid out until every size is known.
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
                // Both licences are computed from the content id.
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
                // The key blobs the hardware unwraps to reach the filesystem. `0x10` is stored
                // in the clear; `0x20` is encrypted below like the licences.
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

    /// Write the header, the entry table and every entry body into the prepared buffer, then
    /// the digests over them.
    fn emit(
        &self,
        buffer: Vec<u8>,
        contents: &[(u32, Vec<u8>)],
        entries: &[Entry],
        table_at: usize,
        gaps: Vec<Gap>,
    ) -> Result<Built, WriteError> {
        let image_at = IMAGE_OFFSET;
        let content_id = &self.content_id;
        let kind = (self.drm_type, self.content_type, self.sku_flag);
        let count = entries.len();
        let mut out = buffer;
        out.get_mut(..MAGIC.len())
            .ok_or(WriteError::TooLarge)?
            .copy_from_slice(&MAGIC);
        write_be(
            &mut out,
            COUNT_AT,
            u32::try_from(count).map_err(|_| WriteError::TooLarge)?,
        );
        write_be(
            &mut out,
            TABLE_AT,
            u32::try_from(table_at).map_err(|_| WriteError::TooLarge)?,
        );
        write_be(&mut out, IMAGE_AT, image_at.try_into().unwrap_or(u64::MAX));

        write_header_fields(
            &mut out,
            count,
            entries,
            image_at,
            self.image.len(),
            kind,
            self.cache_size,
        )?;

        let id = content_id.as_bytes();
        if let Some(slot) = out.get_mut(CONTENT_ID_AT..CONTENT_ID_AT.saturating_add(id.len())) {
            slot.copy_from_slice(id);
        }

        for (slot, entry) in entries.iter().enumerate() {
            let record = table_at.saturating_add(slot.saturating_mul(ENTRY_SIZE));
            write_slice(&mut out, record, &entry.row());
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
        // and the whole-header digest and signature.
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
        // The fixed word every package examined carries.
        if let Some(slot) =
            out.get_mut(derive::manifest::FIXED_1C..derive::manifest::FIXED_1C.saturating_add(4))
        {
            slot.copy_from_slice(&derive::manifest::FIXED_1C_VALUE.to_be_bytes());
        }
        // GameDigest (the image) and ParamDigest (the param.sfo).
        put_digest(&mut out, derive::manifest::IMAGE_DIGEST, &self.image);
        if let Some((_, sfo)) = contents.iter().find(|(id, _)| *id == entry_id::PARAM_SFO) {
            put_digest(&mut out, derive::manifest::PARAM_SFO_DIGEST, sfo);

            // ContentDigest and MajorParamDigest, computed from the param.sfo per LibOrbisPkg;
            // the hardware reads them as the content's identity. HeaderDigest (`0x60`) hashes
            // header fields `emit` has not written yet, so `finalize_digests` fills it.
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
        // HeaderDigest, filled once the header is written.
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
        // Fixed-width: a longer id is truncated and a shorter one leaves zeroes. `build` has
        // already validated the length.
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

/// Place each body from the body offset in layout rank order, sixteen-byte aligned, and build
/// the entry table sorted by id. Returns the entries and where the table goes: the table copy
/// entry `0x100` is the entry table.
fn lay_out(contents: &mut [(u32, Vec<u8>)]) -> Result<(Vec<Entry>, usize), WriteError> {
    contents.sort_by_key(|(id, _)| layout_rank(*id));

    let mut at = usize::try_from(header_value::BODY_OFFSET).unwrap_or(0x2000);
    let mut table_at = HEADER_RESERVED;
    let mut entries = Vec::with_capacity(contents.len());
    for (id, body) in contents.iter() {
        let offset = u32::try_from(at).map_err(|_| WriteError::TooLarge)?;
        let size = u32::try_from(body.len()).map_err(|_| WriteError::TooLarge)?;
        if *id == derive::entry::TABLE_COPY {
            table_at = at;
        }
        let (flags1, flags2) = entry_flags(*id);
        entries.push(Entry {
            id: *id,
            name_offset: resolve_name_offset(*id, contents),
            flags1,
            flags2,
            offset,
            size,
        });
        at = at
            .checked_add(body.len())
            .ok_or(WriteError::TooLarge)?
            .saturating_add(0xF)
            & !0xF;
    }
    entries.sort_by_key(|e| e.id);
    if at > IMAGE_OFFSET {
        return Err(WriteError::TooLarge);
    }
    Ok((entries, table_at))
}

/// The two flag words of an entry's table row.
///
/// One arm per measured id, even where two agree.
#[allow(clippy::match_same_arms)]
fn entry_flags(id: u32) -> (u32, u32) {
    match id {
        0x0001 => (0x4000_0000, 0),
        0x0010 => (0x6000_0000, 0),
        0x0020 => (0xE000_0000, keys::IMAGE_KEY_INDEX << 12),
        0x0080 => (0x6000_0000, 0),
        0x0100 => (0x6000_0000, 0),
        0x0200 => (0x4000_0000, 0),
        entry_id::LICENSE_DAT => (keys::FLAG_ENCRYPTED, LICENCE_KEY_INDEX << 12),
        entry_id::LICENSE_INFO => (keys::FLAG_ENCRYPTED, LICENCE_INFO_KEY_INDEX << 12),
        _ => (0, 0),
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
    /// Empty means nothing was left blank, not that the package is correct.
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
/// Exact, because the image sits at the fixed [`IMAGE_OFFSET`].
fn playgo_len(image: &[u8]) -> usize {
    IMAGE_OFFSET
        .saturating_add(image.len())
        .checked_div(derive::PLAYGO_BLOCK)
        .unwrap_or(0)
        .saturating_mul(derive::PLAYGO_SLOT)
}

/// Encrypt the licence entries in place, before anything digests them.
///
/// The digest table covers what a package stores, which is the ciphertext.
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
        let row = entry.row();
        if let Some((_, body)) = contents.iter_mut().find(|(id, _)| *id == entry.id) {
            // The builder's passcode, so the entries match the package's own key blobs.
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

/// The order a real package lists its entry names in.
///
/// Not ascending entry id: a real package's table reads
/// `\0icon0.png\0param.sfo\0playgo-chunk.dat\0playgo-chunk.sha\0playgo-manifest.xml\0`.
/// `name_offset` makes any order self-consistent; this one matches real packages. Ids not
/// listed follow in the order the package carries them.
const NAME_TABLE_ORDER: [u32; 5] = [0x1200, 0x1000, 0x1001, 0x1002, 0x1003];

/// Build entry `0x200`, the entry name table, from the entries the package actually carries.
///
/// A NUL-separated list of the *named* entries' filenames, opening with a NUL and closing with
/// one. `entry_record.name_offset` points into it. It is a pure function of which entries are
/// present, with names from [`entry_name`].
fn entry_names_table(ids: &[u32]) -> Vec<u8> {
    let mut ordered: Vec<u32> = NAME_TABLE_ORDER
        .iter()
        .copied()
        .filter(|wanted| ids.contains(wanted))
        .collect();
    for id in ids {
        if entry_name(*id).is_some() && !ordered.contains(id) {
            ordered.push(*id);
        }
    }

    let mut out = vec![0_u8];
    for id in ordered {
        if let Some(name) = entry_name(id) {
            out.extend_from_slice(name.as_bytes());
            out.push(0);
        }
    }
    out
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
            // Bytes are not a display string, so both byte kinds render empty.
            Value::Binary(_) | Value::Unknown(..) => String::new(),
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

/// Write the header fields past the magic, count, table offset and image offset.
///
/// Offsets and constants are measured from a real package and cross-checked against
/// `LibOrbisPkg@6434772`'s writer.
fn write_header_fields(
    out: &mut [u8],
    count: usize,
    entries: &[Entry],
    image_at: usize,
    image_len: usize,
    kind: (u16, u16, u16),
    cache_size: Option<u32>,
) -> Result<(), WriteError> {
    let (drm_type, content_type, _sku) = kind;
    let image_len64 = u64::try_from(image_len).map_err(|_| WriteError::TooLarge)?;
    let image_at64 = u64::try_from(image_at).map_err(|_| WriteError::TooLarge)?;
    let total = image_at64
        .checked_add(image_len64)
        .ok_or(WriteError::TooLarge)?;

    write_be(out, header::FLAGS, header_value::FLAGS);
    write_be(out, header::UNK_0C, header_value::UNK_0C);
    // Not the entry count: the count of SC entries, the format's own ahead of the title's. The
    // installer rejects a package with the total here.
    write_be(out, header::SC_ENTRY_COUNT, header_value::SC_ENTRY_COUNT);
    write_be(
        out,
        header::ENTRY_COUNT_2,
        u16::try_from(count).unwrap_or(u16::MAX),
    );
    // The summed size of the SC entries rounded down to 512 bytes (measured: 3659 -> 3584,
    // 4347 -> 4160), not the distance to the image.
    let sc_data: usize = entries
        .iter()
        .take(usize::from(header_value::SC_ENTRY_COUNT))
        .map(|entry| usize::try_from(entry.size).unwrap_or(0))
        .sum();
    write_be(
        out,
        header::MAIN_ENTRY_DATA_SIZE,
        u32::try_from(sc_data & !0x1FF).unwrap_or(0),
    );
    write_be(out, header::BODY_OFFSET, header_value::BODY_OFFSET);
    // The body runs from `BODY_OFFSET` to the image: `0x7E000` with the image at `0x80000`,
    // `0x57E000` with it at `0x580000`. `BODY_DIGEST` hashes exactly this region.
    let body_size = image_at
        .checked_sub(usize::try_from(header_value::BODY_OFFSET).unwrap_or(0))
        .and_then(|len| u64::try_from(len).ok())
        .unwrap_or(header_value::BODY_SIZE);
    write_be(out, header::BODY_SIZE, body_size);
    write_be(out, header::DRM_TYPE, u32::from(drm_type));
    write_be(out, header::CONTENT_TYPE, u32::from(content_type));
    write_be(out, header::CONTENT_FLAGS, header_value::CONTENT_FLAGS);
    // What the installer promotes: everything ahead of the image. It equals the image offset
    // in every package examined; zero means nothing to promote.
    write_be(
        out,
        header::PROMOTE_SIZE,
        u32::try_from(image_at).unwrap_or(0),
    );
    write_be(out, header::VERSION_DATE, header_value::VERSION_DATE);
    write_be(out, header::VERSION_HASH, header_value::VERSION_HASH);
    write_be(out, header::EKC_VERSION, header_value::EKC_VERSION);

    write_be(out, header::UNK_400, header_value::UNK_400);
    write_be(out, header::IMAGE_COUNT, header_value::IMAGE_COUNT);
    write_be(out, header::PFS_FLAGS, header_value::PFS_FLAGS);
    write_be(out, header::IMAGE_SIZE, image_len64);
    write_be(out, header::MOUNT_IMAGE_OFFSET, 0_u64);
    write_be(out, header::MOUNT_IMAGE_SIZE, total);
    write_be(out, header::PACKAGE_SIZE, total);
    write_be(out, header::SIGNED_SIZE, header_value::SIGNED_SIZE);
    write_be(
        out,
        header::CACHE_SIZE,
        cache_size.unwrap_or(header_value::CACHE_SIZE),
    );

    // Two digests over the image, which nothing written after this point touches.
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
    // The entry-data digests (`0x100`-`0x17F`) and the header digest and signature (`0xFE0`,
    // `0x1000`) cover the entry bodies, so `finalize_digests` writes them at the end of `emit`.
    Ok(())
}

/// The digests and signature that cover the whole assembled package.
///
/// Called once the header, entry table, entry bodies and image are all in `out`. The order
/// follows `LibOrbisPkg`'s `CalcBodyDigests`, then the header digest and signature
/// (`PkgBuilder.cs`):
///
/// ```text
/// 0x100   sc_entries1        SHA-256 of SC entry bodies 0x10, 0x20, 0x80, 0x100, 0x1
/// 0x120   sc_entries2        the same minus 0x1, with 0x100 truncated to sc_entry_count * 0x20
/// 0x140   digest_table_hash  SHA-256 of entry 0x1
/// 0x160   body_digest        SHA-256 of the body region
/// 0xFE0   header digest      SHA-256 of out[0..0xFE0]
/// 0x1000  signature          SHA-256 of out[0..0x1000], wrapped under pkg public key 3
/// ```
// One function, because each digest covers bytes an earlier one wrote and the order must hold.
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
    // block. This changes the manifest body, so the digest table is recomputed below.
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

    // body_digest: the whole body region, with the length read back from the header so the
    // digest covers exactly what the header declares.
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
/// Not ascending id. The format's own entries come first (keys, image key, general digests, the
/// metas table, the digest table), then the entry names, the playgo trio, the licences,
/// `param.sfo`, the reserved block and the icon. The first four are `main_ent_data_size` long,
/// which puts the metas entry at `0x2A80`. An unlisted id sorts to the end.
fn layout_rank(id: u32) -> usize {
    const ORDER: [u32; 14] = [
        0x10, 0x20, 0x80, 0x100, 0x1, 0x200, 0x1001, 0x1002, 0x1003, 0x400, 0x401, 0x1000, 0x409,
        0x1200,
    ];
    ORDER.iter().position(|&x| x == id).unwrap_or(ORDER.len())
}

/// Why a package could not be assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WriteError {
    /// Required entries this crate cannot compute and the caller did not supply, by id.
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

/// How large the *inner* filesystem inside an outer image is, or `None` if it cannot be read.
///
/// The inner image is a `PFSC` container held as a file inside the encrypted outer filesystem;
/// the key comes from the content id and passcode. Only the `PFSC` header is read, which records
/// the length its contents decompress to.
///
/// The declared cache size is compared against this, not the larger outer image. The same
/// number is `inner_mchunk_attrs[0].size` in [`crate::playgo::chunk_dat`].
#[must_use]
pub fn inner_image_size(image: &[u8], content_id: &str, passcode: &[u8]) -> Option<u64> {
    use selfish_pfs::{Filesystem, Slice, Source, Xts};

    let ekpfs = keys::derive_filesystem_key(content_id.as_bytes(), passcode);
    let source = Slice::new(image, 0);
    // The superblock, which carries the seed, is in the clear.
    let superblock = source.read(0, 0x400).ok()?;
    let block_size = u64::from(u32::from_le_bytes([
        *superblock.get(0x20)?,
        *superblock.get(0x21)?,
        *superblock.get(0x22)?,
        *superblock.get(0x23)?,
    ]));
    let (tweak, data) = selfish_pfs::image_keys(&ekpfs, &superblock).ok()?;
    let sectors = block_size.checked_div(selfish_pfs::SECTOR_SIZE)?;
    let decrypted = Xts::new(source, &tweak, &data, sectors).ok()?;
    let outer = Filesystem::new(&decrypted).ok()?;
    for found in outer.walk(0).ok()? {
        if !found.path.ends_with(selfish_pfs::outer::IMAGE_NAME) {
            continue;
        }
        let contents = outer.contents(found.inode).ok()?;
        let raw = contents.get(0x28..0x30)?;
        let mut value = [0_u8; 8];
        value.copy_from_slice(raw);
        return Some(u64::from_le_bytes(value));
    }
    None
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use sha2::{Digest as _, Sha256};

    use super::{Builder, WriteError, entry_name, entry_names_table, resolve_name_offset};
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

    /// The header's body size runs from the body offset to the image.
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

    /// The cache size can be set below the default, and the default is written otherwise.
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

    /// A built package parses back with its entries and image offset.
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

    /// Every derived entry claim holds on a package this crate built.
    #[test]
    fn the_derivation_holds_on_a_package_this_crate_built() {
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

    /// The manifest's image digest is SHA-256 of the supplied image.
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

    /// A built package's licence entries are flagged, decrypt, and the licence verifies.
    #[test]
    fn the_licence_in_a_built_package_decrypts_and_verifies() {
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

    /// A missing title entry fails the build by id instead of being zero-filled.
    #[test]
    fn a_missing_entry_is_named_rather_than_zero_filled() {
        let error = Builder::new().image(vec![0; 16]).build().unwrap_err();
        let WriteError::Missing(missing) = &error else {
            panic!("expected a missing-entry error, got {error:?}");
        };
        assert!(missing.contains(&entry_id::PARAM_SFO), "param.sfo");
        for computed in [0x1_u32, 0x80, 0x100, 0x400, 0x401, 0x1002] {
            assert!(
                !missing.contains(&computed),
                "{computed:#x} is computed, not demanded"
            );
        }
        assert!(error.to_string().contains("0x1000"));
    }

    /// Supplying an entry the builder computes is refused.
    #[test]
    fn a_computed_entry_cannot_also_be_supplied() {
        let error = supplied(Builder::new())
            .entry(derive::entry::DIGESTS, vec![0; 32])
            .build()
            .unwrap_err();
        assert_eq!(error, WriteError::AlreadyComputed(derive::entry::DIGESTS));
    }

    /// Unfilled manifest slots are reported as gaps on the built package.
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

    /// The entry name table matches a real package's byte for byte, icon first.
    #[test]
    fn the_entry_name_table_matches_the_one_a_real_package_carries() {
        let table = entry_names_table(&[0x1000, 0x1001, 0x1002, 0x1003, 0x1200]);
        assert_eq!(
            table,
            b"\0icon0.png\0param.sfo\0playgo-chunk.dat\0playgo-chunk.sha\0playgo-manifest.xml\0"
        );
        assert_eq!(table.len(), 75, "the length a real package's table has");
    }

    /// Each entry's name offset points at its own name in the table.
    #[test]
    fn a_name_offset_lands_on_the_name_it_belongs_to() {
        let table = entry_names_table(&[0x1000, 0x1001, 0x1002, 0x1003, 0x1200]);
        let contents = vec![(0x200_u32, table.clone())];
        for id in [0x1000_u32, 0x1001, 0x1002, 0x1003, 0x1200] {
            let at = resolve_name_offset(id, &contents) as usize;
            let name = entry_name(id).expect("a named entry");
            let end = at.saturating_add(name.len());
            assert_eq!(
                table.get(at..end).map(String::from_utf8_lossy).as_deref(),
                Some(name),
                "entry {id:#x} name offset {at} does not land on {name}",
            );
        }
    }

    /// Entries without a filename, such as `0x200` and the digests, are not listed.
    #[test]
    fn entries_without_a_name_are_left_out_of_the_table() {
        let table = entry_names_table(&[0x200, 0x1000, 0x0080, 0x1200]);
        assert_eq!(table, b"\0icon0.png\0param.sfo\0");
    }
}
