//! Build a whole package from a tree of files, then read it back.
//!
//! Every layer of this had been checked on its own. That is not the same claim: a package is
//! four formats nested, and each one can be individually right while the seams between them are
//! wrong - an image placed at the wrong offset, a key derived from the wrong id, an entry whose
//! size disagrees with its contents. Nothing catches that except going all the way through.
//!
//! So this starts with a directory of files and ends by walking the same files back out of the
//! finished package, through the package reader, the decryptor, the outer filesystem, the
//! container and the inner filesystem in turn.
//!
//! # What a pass does not mean
//!
//! It does not mean a console will accept the result. **Nothing this repository has produced
//! has ever been near one.** What it means is that the bytes are self-consistent across every
//! layer, and that the entries a package must carry are present and correctly sized.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing, which is what a test is for"
)]

use selfish_pfs::{Compressed, Filesystem, Region, Slice, Xts, outer, pfsc, write as pfs};
use selfish_pkg::{Package, entry_id, keys, write};

/// The block size every real image uses.
const BLOCK: u32 = 0x10000;
/// Sectors at the front of an image that are left in the clear.
const PLAIN_SECTORS: u64 = 16;
/// A content id of the right length, obviously not a real one.
const CONTENT_ID: &str = "IV0002-SELF00001_00-SELFISHTEST00000";

/// The entries a package must carry that are the *title's* content rather than the format's.
///
/// They are supplied as placeholders here on purpose. A format library that generated a
/// `param.sfo` or an icon would be inventing the title, not the format - so the builder demands
/// them and this test hands over the smallest thing that satisfies the shape.
/// Most of these ids have no constant, on purpose - this crate names only the ones it can
/// identify from evidence, and an invented name would be worse than a number. They are written
/// out here with what is actually known about each.
fn title_entries(builder: write::Builder) -> write::Builder {
    // The key blobs at 0x10 and 0x20 are *not* here any more: the builder computes them from
    // the content id and the passcode. They used to be supplied as zeros, which is a package a
    // console cannot open - see the test below that unwraps them back out.
    builder
        // 0x200 - the entry name table.
        .entry(0x200, vec![0_u8; 256])
        // The title metadata table, and the icon.
        .entry(entry_id::PARAM_SFO, b"\0PSF placeholder".to_vec())
        .entry(0x1200, b"\x89PNG placeholder".to_vec())
        // 0x1001 and 0x1003 - the playgo chunk data and its manifest.
        .entry(0x1001, vec![0_u8; 256])
        .entry(0x1003, b"<psproject/>".to_vec())
}

/// Build the filesystem image a package carries, from a tree of files.
fn image(tree: &pfs::Tree) -> Vec<u8> {
    let inner = pfs::build(tree, BLOCK).expect("an inner filesystem");
    let container = pfsc::wrap(&inner, BLOCK).expect("a container");
    let ekpfs = keys::derive_filesystem_key(CONTENT_ID.as_bytes(), keys::FAKE_PASSCODE);
    outer::build(&outer::Options {
        payload: &container,
        ekpfs: &ekpfs,
        seed: [0; 16],
        encrypt: true,
        block_size: BLOCK,
    })
    .expect("an outer filesystem")
}

#[test]
fn a_package_built_from_a_tree_of_files_reads_back_as_those_files() {
    let tree = pfs::Tree::new(pfs::ROOT_NAME)
        .with_file("eboot.bin", vec![0xE1; 4096])
        .with_dir(
            pfs::Tree::new("sce_sys")
                .with_file("param.sfo", b"\0PSF placeholder".to_vec())
                .with_file("icon0.png", vec![0x89; 512]),
        )
        .with_dir(pfs::Tree::new("sce_module").with_file("libc.prx", vec![0xC0; 20_000]));

    let built = title_entries(
        write::Builder::new()
            .content_id(CONTENT_ID)
            .image(image(&tree)),
    )
    .build()
    .expect("a package");

    // ---- the package, as the package reader sees it ------------------------------------
    let package = Package::parse(&built.bytes).expect("a readable package");
    assert_eq!(
        package.content_id(),
        CONTENT_ID.as_bytes(),
        "the id a reader finds is the one the keys were derived from"
    );
    assert!(
        package.missing_expected_entries().is_empty(),
        "every entry a package must carry: missing {:?}",
        package.missing_expected_entries()
    );

    // ---- down through every nested layer -----------------------------------------------
    let at = package.image_offset().expect("an image offset");
    assert_eq!(
        at, built.image_at,
        "the reader and the writer agree where it is"
    );

    let header = &built.bytes[usize::try_from(at).unwrap()..usize::try_from(at).unwrap() + 0x400];
    let ekpfs = keys::derive_filesystem_key(CONTENT_ID.as_bytes(), keys::FAKE_PASSCODE);
    let (tweak, data) = selfish_pfs::image_keys(&ekpfs, header).expect("the image keys");

    let length = u64::try_from(built.bytes.len()).unwrap() - at;
    let outer_fs = Filesystem::new(
        Xts::new(
            Region::new(Slice::new(&built.bytes, 0), at, length),
            &tweak,
            &data,
            PLAIN_SECTORS,
        )
        .expect("a decryptor"),
    )
    .expect("the outer filesystem");

    let found = outer_fs.walk(pfs::ROOT_INODE).expect("a walk");
    assert_eq!(found.len(), 1, "an outer filesystem holds exactly one file");
    assert!(found[0].path.ends_with(outer::IMAGE_NAME));

    let container = outer_fs.contents(found[0].inode).expect("the container");
    let inner_fs = Filesystem::new(Compressed::new(Slice::new(&container, 0)).expect("a reader"))
        .expect("the inner filesystem");

    let mut paths: Vec<String> = inner_fs
        .walk(pfs::ROOT_INODE)
        .expect("a walk")
        .into_iter()
        .map(|f| f.path)
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "/eboot.bin",
            "/sce_module/libc.prx",
            "/sce_sys/icon0.png",
            "/sce_sys/param.sfo"
        ],
        "the files that went in are the files that come out"
    );
}

#[test]
fn the_bytes_of_every_file_survive_the_whole_nest() {
    // Paths surviving is weaker than it looks - an inode can point at the right name and the
    // wrong blocks. This checks contents, and picks sizes that straddle block boundaries.
    let big: Vec<u8> = (0..80_000_u32)
        .map(|n| u8::try_from(n & 0xFF).unwrap())
        .collect();
    let tree = pfs::Tree::new(pfs::ROOT_NAME)
        .with_file("big", big.clone())
        .with_file("empty", Vec::new())
        .with_file("one", b"1".to_vec());

    let built = title_entries(
        write::Builder::new()
            .content_id(CONTENT_ID)
            .image(image(&tree)),
    )
    .build()
    .expect("a package");

    let package = Package::parse(&built.bytes).expect("a readable package");
    let at = package.image_offset().expect("an offset");
    let header = &built.bytes[usize::try_from(at).unwrap()..usize::try_from(at).unwrap() + 0x400];
    let ekpfs = keys::derive_filesystem_key(CONTENT_ID.as_bytes(), keys::FAKE_PASSCODE);
    let (tweak, data) = selfish_pfs::image_keys(&ekpfs, header).expect("keys");
    let length = u64::try_from(built.bytes.len()).unwrap() - at;

    let outer_fs = Filesystem::new(
        Xts::new(
            Region::new(Slice::new(&built.bytes, 0), at, length),
            &tweak,
            &data,
            PLAIN_SECTORS,
        )
        .expect("a decryptor"),
    )
    .expect("the outer filesystem");
    let entry = outer_fs.walk(pfs::ROOT_INODE).expect("a walk").remove(0);
    let container = outer_fs.contents(entry.inode).expect("the container");
    let inner_fs = Filesystem::new(Compressed::new(Slice::new(&container, 0)).expect("a reader"))
        .expect("the inner filesystem");

    for found in inner_fs.walk(pfs::ROOT_INODE).expect("a walk") {
        let bytes = inner_fs.contents(found.inode).expect("bytes");
        let want: &[u8] = match found.path.as_str() {
            "/big" => &big,
            "/empty" => b"",
            "/one" => b"1",
            other => panic!("unexpected {other}"),
        };
        assert_eq!(
            bytes, want,
            "{} differs after four nested formats",
            found.path
        );
    }
}

#[test]
fn the_key_blobs_in_a_built_package_lead_back_to_the_filesystem_key() {
    // **This is the test that says a console could open it.**
    //
    // `keys::filesystem_key` is the reading path, and it does not take the short cut this
    // crate's own tests otherwise take. It goes the way a console goes: pull the wrapped `dk3`
    // out of entry 0x10, RSA-unwrap it, use it to derive an AES key from entry 0x20's *table
    // row*, decrypt that entry, then RSA-unwrap what is inside to reach the filesystem key.
    //
    // Every step of that consults the blobs the builder just produced. Zeros there - which is
    // what this crate emitted until now - fail at the first RSA padding check.
    let built = title_entries(write::Builder::new().content_id(CONTENT_ID).image(image(
        &pfs::Tree::new(pfs::ROOT_NAME).with_file("eboot.bin", vec![1; 16]),
    )))
    .build()
    .expect("a package");

    let package = Package::parse(&built.bytes).expect("a readable package");
    let recovered = keys::filesystem_key(&package).expect("the filesystem key, the long way");
    let computed = keys::derive_filesystem_key(CONTENT_ID.as_bytes(), keys::FAKE_PASSCODE);
    assert_eq!(
        recovered.as_slice(),
        computed.as_slice(),
        "the key unwrapped from the package must be the key its filesystem was built with"
    );
}

#[test]
fn a_caller_cannot_supply_the_key_blobs_by_hand() {
    // Two sources for one entry is how a package ends up with a key blob that does not match
    // the filesystem beside it. The builder refuses rather than silently preferring one.
    let attempt = title_entries(write::Builder::new().content_id(CONTENT_ID).image(image(
        &pfs::Tree::new(pfs::ROOT_NAME).with_file("eboot.bin", vec![1; 16]),
    )))
    .entry(entry_id::IMAGE_KEY, vec![0_u8; 0x100])
    .build();
    assert!(
        attempt.is_err(),
        "supplying a computed entry must be refused"
    );
}

#[test]
fn a_package_keyed_with_another_passcode_still_opens_with_that_passcode() {
    // The passcode is an input, and the whole chain has to follow it: the filesystem key, the
    // blobs, and the entry encryption all move together or the package is inconsistent.
    let passcode = b"anotherpasscodethirtytwocharslong";
    let passcode = &passcode[..32];
    let tree = pfs::Tree::new(pfs::ROOT_NAME).with_file("eboot.bin", vec![9; 32]);
    let inner = pfs::build(&tree, BLOCK).expect("an inner filesystem");
    let container = pfsc::wrap(&inner, BLOCK).expect("a container");
    let ekpfs = keys::derive_filesystem_key(CONTENT_ID.as_bytes(), passcode);
    let outer_image = outer::build(&outer::Options {
        payload: &container,
        ekpfs: &ekpfs,
        seed: [0; 16],
        encrypt: true,
        block_size: BLOCK,
    })
    .expect("an outer filesystem");

    let built = title_entries(
        write::Builder::new()
            .content_id(CONTENT_ID)
            .passcode(passcode)
            .image(outer_image),
    )
    .build()
    .expect("a package");

    let package = Package::parse(&built.bytes).expect("a readable package");
    let recovered = keys::filesystem_key(&package).expect("the filesystem key");
    assert_eq!(recovered.as_slice(), ekpfs.as_slice());
    assert_ne!(
        recovered.as_slice(),
        keys::derive_filesystem_key(CONTENT_ID.as_bytes(), keys::FAKE_PASSCODE).as_slice(),
        "and it is genuinely not the fake-passcode key"
    );
}

#[test]
fn the_licence_in_a_built_package_still_verifies() {
    // The licence is the one thing in a package that carries a real signature, and it is
    // computed over fields the builder fills in. If the content id reached it wrongly, or the
    // entry was stored without its encryption flag, this is where it shows.
    let built = title_entries(write::Builder::new().content_id(CONTENT_ID).image(image(
        &pfs::Tree::new(pfs::ROOT_NAME).with_file("eboot.bin", vec![1; 16]),
    )))
    .build()
    .expect("a package");

    let package = Package::parse(&built.bytes).expect("a readable package");
    let entry = package
        .entry(entry_id::LICENSE_DAT)
        .expect("a licence entry");
    let plain = keys::decrypt_entry(&package, entry).expect("a decrypted licence");
    assert_eq!(&plain[..4], b"RIF\0", "a licence starts with its magic");
}

#[test]
fn the_only_thing_a_built_package_cannot_fill_is_the_three_unexplained_digests() {
    // **This is the honest statement of how complete package support is**, pinned as a test so
    // it cannot quietly drift in either direction.
    //
    // Everything else in a package is either computed or supplied by the caller. What is left
    // is three thirty-two-byte slots in entry `0x80`, each of which digests something that has
    // never been found anywhere in a package - checked against every entry, every file region,
    // all the extracted files, under both SHA-256 and SHA-1.
    //
    // They are left blank rather than filled with a guess (principle 5), and the builder says
    // so rather than reporting success. If somebody works out what they digest, this test
    // fails, and that is the point: it is the marker for the last unknown.
    let built = title_entries(write::Builder::new().content_id(CONTENT_ID).image(image(
        &pfs::Tree::new(pfs::ROOT_NAME).with_file("eboot.bin", vec![1; 16]),
    )))
    .build()
    .expect("a package");

    assert_eq!(built.gaps.len(), 3, "gaps: {:?}", built.gaps);
    for gap in &built.gaps {
        assert_eq!(gap.entry, 0x80, "gap outside 0x80: {gap:?}");
        assert_eq!(gap.length, 32, "each is one digest: {gap:?}");
    }
    let offsets: Vec<usize> = built.gaps.iter().map(|gap| gap.offset).collect();
    assert_eq!(
        offsets,
        [32, 96, 160],
        "the same three slots, in the same places, as in every package examined"
    );
    assert!(
        !built.is_complete(),
        "a package with holes must not report itself complete"
    );
}
