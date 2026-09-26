//! `param.json` - what Prospero-generation titles carry instead of `PARAM.SFO`.
//!
//! Named accessors cover the fields with a citable meaning; the parsed document underneath is
//! kept verbatim, so every other key survives a round trip and none is guessed at.
//!
//! The title name is per locale: `localizedParameters` maps a locale to a block containing
//! `titleName`, and its `defaultLanguage` names the one to prefer.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A parsed `param.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Param {
    document: Map<String, Value>,
}

impl Param {
    /// An empty document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            document: Map::new(),
        }
    }

    /// Read one.
    ///
    /// # Errors
    ///
    /// If the bytes are not JSON, or are JSON that is not an object.
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Write it back.
    ///
    /// Two-space indented, as the measured files are. Key order is preserved from the
    /// document as read.
    ///
    /// # Errors
    ///
    /// If a value cannot be serialised, which for a document that was parsed cannot happen.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }

    /// The whole document, for anything this module does not name.
    #[must_use]
    pub const fn document(&self) -> &Map<String, Value> {
        &self.document
    }

    /// The whole document, mutably.
    pub const fn document_mut(&mut self) -> &mut Map<String, Value> {
        &mut self.document
    }

    /// The title id, such as `PPSA01650`.
    #[must_use]
    pub fn title_id(&self) -> Option<&str> {
        self.document.get("titleId")?.as_str()
    }

    /// The content id, such as `UP0000-PPSA01650_00-YOUTUBE000000000`.
    #[must_use]
    pub fn content_id(&self) -> Option<&str> {
        self.document.get("contentId")?.as_str()
    }

    /// The application category, where the file states one.
    ///
    /// See [`category`] for the known values. Accepts a string as well as a number: the
    /// vendor's files use a number, and dumping tools write both.
    #[must_use]
    pub fn category(&self) -> Option<i64> {
        let value = self.document.get("applicationCategoryType")?;
        value
            .as_i64()
            .or_else(|| value.as_str()?.trim().parse().ok())
    }
}

/// Application category types (`applicationCategoryType` in `param.json`).
///
/// The category sets the direct-memory budget, video-out bus ownership and process
/// lifecycle. It is independent of process privilege (`paid`, the authority id).
pub mod category {
    /// Big app, a native game. Full direct-memory budget (about 12.5 GB on Prospero-generation
    /// and 5.5 GB on Orbis-generation hardware) and exclusive primary scanout (video bus 0).
    /// Foreground-exclusive; the dynamic linker requires `/app0/sce_module/libc.prx`, and
    /// launch is gated by a `PFAuthClient` entitlement check.
    pub const BIG_APP: i64 = 0;

    /// System app. No direct memory is granted (userland `mmap`/`malloc` only), and
    /// `sceVideoOutOpen` on the primary scanout fails with `0x80290001`. Runs as a background
    /// utility and needs no `libc.prx`.
    pub const SYSTEM_APP: i64 = 0x10000;

    /// Mini app. A constrained direct-memory budget (about 256-512 MB) and a secondary
    /// compositor layer; runs alongside a big app without preempting it.
    pub const MINI_APP: i64 = 0x20000;

    /// Daemon. A minimal memory pool and no video out.
    pub const DAEMON: i64 = 3;

    /// Media app. Its own media-streaming memory budget and a protected video path.
    pub const MEDIA_APP: i64 = 0x40000;
}

impl Param {
    /// The locale named as the default, if the file names one.
    #[must_use]
    pub fn default_language(&self) -> Option<&str> {
        self.localized()?.get("defaultLanguage")?.as_str()
    }

    /// The title name in the default language, falling back to any locale that has one.
    ///
    /// The fallback covers a file that carries locales without naming a default.
    #[must_use]
    pub fn title_name(&self) -> Option<&str> {
        let locales = self.localized()?;
        if let Some(name) = self
            .default_language()
            .and_then(|language| locales.get(language))
            .and_then(|locale| locale.get("titleName"))
            .and_then(Value::as_str)
        {
            return Some(name);
        }
        locales
            .values()
            .find_map(|locale| locale.get("titleName")?.as_str())
    }

    /// The title name in one specific locale.
    #[must_use]
    pub fn title_name_in(&self, language: &str) -> Option<&str> {
        self.localized()?.get(language)?.get("titleName")?.as_str()
    }

    /// The title subtitle in the default language, falling back to any locale that has one.
    #[must_use]
    pub fn title_sub_name(&self) -> Option<&str> {
        let locales = self.localized()?;
        if let Some(name) = self
            .default_language()
            .and_then(|language| locales.get(language))
            .and_then(|locale| locale.get("titleSubName"))
            .and_then(Value::as_str)
        {
            return Some(name);
        }
        locales
            .values()
            .find_map(|locale| locale.get("titleSubName")?.as_str())
    }

    /// The title subtitle in one specific locale.
    #[must_use]
    pub fn title_sub_name_in(&self, language: &str) -> Option<&str> {
        self.localized()?
            .get(language)?
            .get("titleSubName")?
            .as_str()
    }

    /// Every locale the file carries a name for.
    #[must_use]
    pub fn languages(&self) -> Vec<&str> {
        self.localized().map_or_else(Vec::new, |locales| {
            locales
                .iter()
                .filter(|(_, locale)| locale.get("titleName").is_some())
                .map(|(language, _)| language.as_str())
                .collect()
        })
    }

    /// The application version string, where stated (e.g. "01.00").
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.document.get("contentVersion")?.as_str()
    }

    /// The master version string, where stated (e.g. "01.00").
    #[must_use]
    pub fn master_version(&self) -> Option<&str> {
        self.document.get("masterVersion")?.as_str()
    }

    /// The SDK version the title was built against, if stated.
    #[must_use]
    pub fn sdk_version(&self) -> Option<&str> {
        self.document.get("sdkVersion")?.as_str()
    }

    /// The deeplink URI for application launch handoff.
    #[must_use]
    pub fn deeplink_uri(&self) -> Option<&str> {
        self.document.get("deeplinkUri")?.as_str()
    }

    /// Whether this metadata document represents a native Prospero title.
    ///
    /// True when the title id carries a Prospero-generation prefix (`PPSA`, `NPXS`, or this
    /// collection's `OBSC`) or the category is a system-level one.
    #[must_use]
    pub fn is_prospero_native(&self) -> bool {
        if self.title_id().is_some_and(|id| {
            id.starts_with("PPSA") || id.starts_with("NPXS") || id.starts_with("OBSC")
        }) {
            return true;
        }
        self.category().is_some_and(|c| c >= 0x10000)
    }

    /// Set the content id.
    pub fn set_content_id(&mut self, content_id: &str) {
        self.document
            .insert("contentId".to_owned(), Value::String(content_id.to_owned()));
    }

    /// Set the content version.
    pub fn set_version(&mut self, version: &str) {
        self.document.insert(
            "contentVersion".to_owned(),
            Value::String(version.to_owned()),
        );
    }

    /// Set the master version.
    pub fn set_master_version(&mut self, version: &str) {
        self.document.insert(
            "masterVersion".to_owned(),
            Value::String(version.to_owned()),
        );
    }

    /// Set the SDK version.
    pub fn set_sdk_version(&mut self, version: &str) {
        self.document
            .insert("sdkVersion".to_owned(), Value::String(version.to_owned()));
    }

    /// Set the deeplink URI.
    pub fn set_deeplink_uri(&mut self, uri: &str) {
        self.document
            .insert("deeplinkUri".to_owned(), Value::String(uri.to_owned()));
    }

    /// Set the fields a homebrew title needs, leaving everything else alone.
    ///
    /// The four fields measured on the hardware and no more; other store metadata has no
    /// citable meaning here and is not guessed at.
    pub fn set_basics(&mut self, title_id: &str, title_name: &str, language: &str, category: i64) {
        self.document
            .insert("titleId".to_owned(), Value::String(title_id.to_owned()));
        self.document.insert(
            "applicationCategoryType".to_owned(),
            Value::Number(category.into()),
        );

        let mut locale = Map::new();
        locale.insert("titleName".to_owned(), Value::String(title_name.to_owned()));

        let locales = self
            .document
            .entry("localizedParameters")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(locales) = locales.as_object_mut() {
            locales.insert(
                "defaultLanguage".to_owned(),
                Value::String(language.to_owned()),
            );
            locales.insert(language.to_owned(), Value::Object(locale));
        }
    }

    /// Configure a full Prospero-generation title descriptor.
    pub fn set_prospero(
        &mut self,
        title_id: &str,
        title_name: &str,
        language: &str,
        category: i64,
        content_id: Option<&str>,
        deeplink: Option<&str>,
    ) {
        self.set_basics(title_id, title_name, language, category);
        if let Some(cid) = content_id {
            self.set_content_id(cid);
        }
        if let Some(link) = deeplink {
            self.set_deeplink_uri(link);
        }
    }

    /// Set the subtitle in a specific locale.
    pub fn set_title_sub_name(&mut self, language: &str, sub_name: &str) {
        let locales = self
            .document
            .entry("localizedParameters")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(locales) = locales.as_object_mut() {
            let locale = locales
                .entry(language.to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(locale) = locale.as_object_mut() {
                locale.insert(
                    "titleSubName".to_owned(),
                    Value::String(sub_name.to_owned()),
                );
            }
        }
    }

    fn localized(&self) -> Option<&Map<String, Value>> {
        self.document.get("localizedParameters")?.as_object()
    }
}

impl Default for Param {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{Param, category};

    const REAL_SHAPE: &str = r#"{
        "titleId": "PPSA01650",
        "contentId": "UP0000-PPSA01650_00-YOUTUBE000000000",
        "applicationCategoryType": 0,
        "somethingUndocumented": {"nested": [1, 2, 3]},
        "localizedParameters": {
            "defaultLanguage": "en-US",
            "ja-JP": {"titleName": "本当"},
            "en-US": {"titleName": "A Name"}
        }
    }"#;

    /// The default language picks the title name, not the first locale in the file.
    #[test]
    fn the_default_language_decides_the_title_and_not_the_first_locale() {
        let param = Param::parse(REAL_SHAPE.as_bytes()).expect("a document");
        assert_eq!(param.title_name(), Some("A Name"));
        assert_eq!(param.default_language(), Some("en-US"));
        assert_eq!(param.title_name_in("ja-JP"), Some("本当"));
    }

    /// A file with no default language still yields a title name.
    #[test]
    fn a_file_without_a_default_still_names_its_title() {
        let param = Param::parse(br#"{"localizedParameters":{"fr-FR":{"titleName":"Un Nom"}}}"#)
            .expect("a document");
        assert_eq!(param.title_name(), Some("Un Nom"));
        assert_eq!(param.default_language(), None);
    }

    /// Keys without an accessor survive a round trip.
    #[test]
    fn keys_this_module_does_not_name_survive_a_round_trip() {
        let param = Param::parse(REAL_SHAPE.as_bytes()).expect("a document");
        let written = param.to_bytes().expect("bytes");
        let again = Param::parse(&written).expect("a document");
        assert_eq!(param, again);
        assert!(again.document().contains_key("somethingUndocumented"));
    }

    /// A category written as a string is read, and a non-numeric one is absent.
    #[test]
    fn a_category_written_as_a_string_is_read_rather_than_dropped() {
        assert_eq!(
            Param::parse(br#"{"applicationCategoryType":0}"#)
                .unwrap()
                .category(),
            Some(0)
        );
        assert_eq!(
            Param::parse(br#"{"applicationCategoryType":" 5 "}"#)
                .unwrap()
                .category(),
            Some(5)
        );
        assert_eq!(
            Param::parse(br#"{"applicationCategoryType":"abc"}"#)
                .unwrap()
                .category(),
            None,
            "and a garbage one is absent rather than failing the file"
        );
    }

    /// `set_basics` writes the four measured fields and nothing else.
    #[test]
    fn the_four_measured_fields_write_a_document_that_reads_back() {
        let mut param = Param::new();
        param.set_basics("TEST00001", "A Homebrew", "en-US", 0);

        let again = Param::parse(&param.to_bytes().expect("bytes")).expect("a document");
        assert_eq!(again.title_id(), Some("TEST00001"));
        assert_eq!(again.title_name(), Some("A Homebrew"));
        assert_eq!(again.default_language(), Some("en-US"));
        assert_eq!(again.category(), Some(0));
        assert_eq!(
            again.document().len(),
            3,
            "three top-level keys, and nothing invented alongside them"
        );
    }

    /// `set_basics` on an existing document leaves other keys and locales alone.
    #[test]
    fn setting_the_basics_leaves_other_keys_alone() {
        let mut param = Param::parse(REAL_SHAPE.as_bytes()).expect("a document");
        param.set_basics("NEW00001", "Renamed", "en-US", 1);

        assert_eq!(param.title_id(), Some("NEW00001"));
        assert_eq!(param.title_name(), Some("Renamed"));
        assert!(param.document().contains_key("somethingUndocumented"));
        assert_eq!(
            param.title_name_in("ja-JP"),
            Some("本当"),
            "another locale is not a field this touched"
        );
    }

    /// `languages` lists locale blocks only, never the `defaultLanguage` sibling key.
    #[test]
    fn languages_lists_only_locales_that_name_a_title() {
        let param = Param::parse(REAL_SHAPE.as_bytes()).expect("a document");
        let mut languages = param.languages();
        languages.sort_unstable();
        assert_eq!(languages, ["en-US", "ja-JP"]);
    }

    /// JSON that is not an object, and text that is not JSON, are refused.
    #[test]
    fn json_that_is_not_an_object_is_refused() {
        assert!(Param::parse(b"[1,2,3]").is_err());
        assert!(Param::parse(b"not json").is_err());
    }

    /// A full Prospero-generation descriptor round-trips and is detected as native.
    #[test]
    fn native_prospero_title_configuration_and_detection() {
        let mut param = Param::new();
        param.set_prospero(
            "PPSA01650",
            "YouTube Prospero",
            "en-US",
            0x10000,
            Some("UP0000-PPSA01650_00-YOUTUBE000000000"),
            Some("prospero://launch/youtube"),
        );
        param.set_version("01.00");
        param.set_master_version("01.00");
        param.set_sdk_version("12.40.00.01");

        assert!(param.is_prospero_native());
        assert_eq!(param.title_id(), Some("PPSA01650"));
        assert_eq!(
            param.content_id(),
            Some("UP0000-PPSA01650_00-YOUTUBE000000000")
        );
        assert_eq!(param.deeplink_uri(), Some("prospero://launch/youtube"));
        assert_eq!(param.version(), Some("01.00"));
        assert_eq!(param.master_version(), Some("01.00"));
        assert_eq!(param.sdk_version(), Some("12.40.00.01"));

        let bytes = param.to_bytes().expect("bytes");
        let parsed = Param::parse(&bytes).expect("document");
        assert_eq!(param, parsed);
        assert!(parsed.is_prospero_native());
    }

    /// The category constants hold their values and `set_basics` writes them.
    #[test]
    fn category_constants_and_set_basics() {
        assert_eq!(category::BIG_APP, 0);
        assert_eq!(category::SYSTEM_APP, 0x10000);
        assert_eq!(category::MINI_APP, 0x20000);
        assert_eq!(category::DAEMON, 3);
        assert_eq!(category::MEDIA_APP, 0x40000);

        let mut param = Param::new();
        param.set_basics("CUSA00001", "Big Game", "en-US", category::BIG_APP);
        assert_eq!(param.category(), Some(category::BIG_APP));

        param.set_basics("CUSA00002", "System Tool", "en-US", category::SYSTEM_APP);
        assert_eq!(param.category(), Some(category::SYSTEM_APP));
    }

    /// A subtitle set in one locale reads back there and through a round trip.
    #[test]
    fn title_sub_name_reads_and_sets() {
        let mut param = Param::new();
        param.set_basics("GLCB00001", "GL1 Cube", "en-US", 0);
        assert_eq!(param.title_sub_name(), None);

        param.set_title_sub_name("en-US", "Freestanding OpenGL 1.3 demonstration");
        assert_eq!(
            param.title_sub_name(),
            Some("Freestanding OpenGL 1.3 demonstration")
        );
        assert_eq!(
            param.title_sub_name_in("en-US"),
            Some("Freestanding OpenGL 1.3 demonstration")
        );
        assert_eq!(param.title_sub_name_in("ja-JP"), None);

        let bytes = param.to_bytes().expect("bytes");
        let again = Param::parse(&bytes).expect("document");
        assert_eq!(
            again.title_sub_name(),
            Some("Freestanding OpenGL 1.3 demonstration")
        );
    }
}
