//! Which keys a package's `PARAM.SFO` carries.
//!
//! The format itself (magic, index, tables, padding) belongs to `selfish-title`, and the table
//! is built with it. `selfish-title` asserts no required-key list; a package is the
//! consumer that supplies one. [`Params`] holds what identifies a title, and [`game`] builds the
//! Prospero-generation field set measured out of real packages.

use selfish_title::sfo::{Entry, Sfo};

/// Values measured out of real packages, each identical in both Prospero-generation packages
/// examined. Fields where the samples disagree are title-specific and are not here.
mod measured {
    /// An application. Orbis-generation packages carry `4` here.
    pub(super) const APP_TYPE: u32 = 1;
    /// The bit both samples set. One of them sets more; that extra is title-specific.
    pub(super) const ATTRIBUTE: u32 = 0x0080_0002;
    /// Identical in both samples, and absent from the Orbis-generation field set.
    pub(super) const ATTRIBUTE2: u32 = 0x400;
    /// Not a development build.
    pub(super) const DEV_FLAG: u32 = 0;
    /// Identical in both samples. An Orbis-generation default writes zero here.
    pub(super) const SYSTEM_VER: u32 = 0x0800_8000;
    /// The publishing tool's version, identical in both samples.
    pub(super) const PUBTOOL_VER: u32 = 0x0289_0000;
    /// The lowest publishing tool version accepted, identical in both samples.
    pub(super) const PUBTOOL_MIN_VER: u32 = 0x0299_0000;
    /// Zero in both samples.
    pub(super) const REMOTE_PLAY_KEY_ASSIGN: u32 = 0;
    /// Zero in both samples.
    pub(super) const USER_DEFINED_PARAM: u32 = 0;
    /// Parental level, `1` in every sample.
    pub(super) const PARENTAL_LEVEL: u32 = 1;
    /// Download size. Both samples carry a title-specific value; zero is the neutral one.
    pub(super) const DOWNLOAD_DATA_SIZE: u32 = 0x100;
}

/// How much room each text field reserves.
///
/// Not the length of the value: a reader takes the field to be this wide whatever it holds,
/// so sizing it to the text would move every value after it.
mod width {
    /// The content id.
    pub(super) const CONTENT_ID: u32 = 48;
    /// The title, as a person reads it.
    pub(super) const TITLE: u32 = 128;
    /// The title id.
    pub(super) const TITLE_ID: u32 = 12;
    /// A version, as `NN.NN`.
    pub(super) const VERSION: u32 = 8;
    /// A four-character code.
    pub(super) const CODE: u32 = 4;
    /// A service id slot.
    pub(super) const SERVICE_ID: u32 = 20;
    /// The publishing tool's note.
    pub(super) const PUBTOOLINFO: u32 = 512;
}

/// How many service id slots a package carries.
const SERVICE_ID_SLOTS: usize = 7;
/// How many user-defined slots it carries.
const USER_DEFINED_SLOTS: usize = 4;

/// What identifies a title, which this crate cannot know.
#[derive(Debug, Clone)]
pub struct Params<'a> {
    /// The content id, thirty-six characters.
    pub content_id: &'a str,
    /// The title id, such as `OBSC00001`. The middle of the content id.
    pub title_id: &'a str,
    /// What the title is called, as a person reads it.
    pub title: &'a str,
    /// The version, as `NN.NN`.
    pub version: &'a str,
}

/// The table a game package carries.
///
/// The Prospero-generation set of twenty-nine fields, not the Orbis-generation twelve of
/// `LibOrbisPkg@6434772`'s default. Every value identifying the title comes from `params`;
/// the rest are measured constants.
#[must_use]
pub fn game(params: &Params<'_>) -> Sfo {
    let mut sfo = Sfo::new();
    let mut entries = vec![
        Entry::integer("APP_TYPE", measured::APP_TYPE),
        Entry::text_reserving("APP_VER", params.version, width::VERSION),
        Entry::integer("ATTRIBUTE", measured::ATTRIBUTE),
        Entry::integer("ATTRIBUTE2", measured::ATTRIBUTE2),
        Entry::text_reserving("CATEGORY", "gd", width::CODE),
        Entry::text_reserving("CONTENT_ID", params.content_id, width::CONTENT_ID),
        Entry::integer("DEV_FLAG", measured::DEV_FLAG),
        Entry::integer("DOWNLOAD_DATA_SIZE", measured::DOWNLOAD_DATA_SIZE),
        Entry::text_reserving("FORMAT", "obs", width::CODE),
        Entry::integer("PARENTAL_LEVEL", measured::PARENTAL_LEVEL),
        Entry::text_reserving(
            "PUBTOOLINFO",
            "c_date=20240101,img0_l0_size=12,img0_l1_size=0,img0_sc_ksize=512,img0_pc_ksize=832",
            width::PUBTOOLINFO,
        ),
        Entry::integer("PUBTOOLMINVER", measured::PUBTOOL_MIN_VER),
        Entry::integer("PUBTOOLVER", measured::PUBTOOL_VER),
        Entry::integer("REMOTE_PLAY_KEY_ASSIGN", measured::REMOTE_PLAY_KEY_ASSIGN),
    ];

    for index in 1..=SERVICE_ID_SLOTS {
        entries.push(Entry::text_reserving(
            &format!("SERVICE_ID_ADDCONT_ADD_{index}"),
            "",
            width::SERVICE_ID,
        ));
    }
    entries.push(Entry::integer("SYSTEM_VER", measured::SYSTEM_VER));
    entries.push(Entry::text_reserving("TITLE", params.title, width::TITLE));
    entries.push(Entry::text_reserving(
        "TITLE_ID",
        params.title_id,
        width::TITLE_ID,
    ));
    for index in 1..=USER_DEFINED_SLOTS {
        entries.push(Entry::integer(
            &format!("USER_DEFINED_PARAM_{index}"),
            measured::USER_DEFINED_PARAM,
        ));
    }
    entries.push(Entry::text_reserving(
        "VERSION",
        params.version,
        width::VERSION,
    ));

    entries.sort_by(|a, b| a.key.cmp(&b.key));
    for entry in entries {
        sfo.set(entry);
    }
    sfo
}

/// The table a game package carries, serialised.
#[must_use]
pub fn game_bytes(params: &Params<'_>) -> Vec<u8> {
    game(params).to_bytes()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing, which is what a test is for"
)]
mod tests {
    use super::{Params, game, game_bytes};
    use selfish_title::sfo::{self, Value};

    fn params() -> Params<'static> {
        Params {
            content_id: "IV0002-OBSC00001_00-OBSCENEPROBE0000",
            title_id: "OBSC00001",
            title: "obSCEne",
            version: "01.00",
        }
    }

    /// The table serialises through `selfish-title` and parses back with the chosen values.
    #[test]
    fn it_serialises_through_the_format_crate_and_reads_back() {
        let bytes = game_bytes(&params());
        assert_eq!(&bytes[..4], b"\0PSF");
        let back = sfo::Sfo::parse(&bytes).expect("a readable table");
        assert_eq!(back.text("TITLE_ID"), Some("OBSC00001"));
        assert_eq!(back.text("TITLE"), Some("obSCEne"));
        assert_eq!(back.text("CATEGORY"), Some("gd"));
    }

    /// The generation-specific fields carry the Prospero-generation measured values.
    #[test]
    fn the_fields_measured_from_real_packages_are_the_measured_values() {
        let sfo = game(&params());
        assert_eq!(sfo.get("APP_TYPE"), Some(&Value::Integer(1)));
        assert_eq!(sfo.get("SYSTEM_VER"), Some(&Value::Integer(0x0800_8000)));
        assert_eq!(sfo.get("ATTRIBUTE2"), Some(&Value::Integer(0x400)));
        assert_eq!(sfo.get("PUBTOOLVER"), Some(&Value::Integer(0x0289_0000)));
        assert_eq!(sfo.get("PUBTOOLMINVER"), Some(&Value::Integer(0x0299_0000)));
    }

    /// The table carries all twenty-nine fields, including the empty service-id slots.
    #[test]
    fn the_field_set_is_the_one_real_packages_carry() {
        let sfo = game(&params());
        let keys: Vec<&str> = sfo.entries().iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys.len(), 29, "{keys:?}");
        for index in 1..=7 {
            assert!(keys.contains(&format!("SERVICE_ID_ADDCONT_ADD_{index}").as_str()));
        }
        for index in 1..=4 {
            assert!(keys.contains(&format!("USER_DEFINED_PARAM_{index}").as_str()));
        }
    }

    /// A text field reserves its full width whatever the text's length.
    #[test]
    fn a_field_reserves_its_full_width_whatever_the_text_is() {
        let short = game_bytes(&Params {
            title: "a",
            ..params()
        });
        let long = game_bytes(&Params {
            title: "a considerably longer title",
            ..params()
        });
        assert_eq!(short.len(), long.len());
    }
}
