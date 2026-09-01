//! `param.json` - what the current generation uses instead of `PARAM.SFO`.
//!
//! JSON, and a large schema of which a handful of fields matter to anything outside the
//! console's own store. Modelled as **the fields this project has grounds for, plus
//! everything else kept verbatim** - see below, because that shape is the whole design.
//!
//! # Why the rest is kept rather than dropped
//!
//! A title's `param.json` carries dozens of keys, most of them store metadata nobody here has
//! a citable meaning for. Two obvious designs both fail:
//!
//! - A struct of known fields **drops** the rest. Read a real file, write it back, and it
//!   comes out shorter than it went in. Principle 4 says a round trip is a test, and this one
//!   would fail on every real file.
//! - Guessing at the unknown keys is the invention principle 5 forbids.
//!
//! So: named accessors for the fields with grounds, and the parsed document underneath,
//! unmodified. A writer emits what it read plus what it changed, and nothing silently
//! disappears.
//!
//! # The title name is not one field
//!
//! `localizedParameters` is a map of locale to a block containing `titleName`, with a
//! `defaultLanguage` naming which one to prefer. Reading the first entry gives a Japanese
//! title for a title that ships in twelve languages - right shape, wrong answer, no error.

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
    /// Two-space indented, which is what the files this was measured against use. The
    /// key order is preserved from the document as read.
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
    /// Accepts a string as well as a number, because dumping tools write both. The console's
    /// own files use a number; a reader that refuses the string form reports a title as
    /// having no category rather than reporting the tool as sloppy.
    #[must_use]
    pub fn category(&self) -> Option<i64> {
        let value = self.document.get("applicationCategoryType")?;
        value
            .as_i64()
            .or_else(|| value.as_str()?.trim().parse().ok())
    }

    /// The locale named as the default, if the file names one.
    #[must_use]
    pub fn default_language(&self) -> Option<&str> {
        self.localized()?.get("defaultLanguage")?.as_str()
    }

    /// The title name in the default language, falling back to any locale that has one.
    ///
    /// The fallback is deliberate and it is second: a file can carry locales without naming a
    /// default, and refusing to name the title at all would be a worse answer than an
    /// arbitrary correct one. Preferring the default when there is one is what stops the
    /// arbitrary case from being the normal case.
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

    /// Whether this metadata document represents a native PS5 title.
    ///
    /// True when the title ID follows the current generation prefix (`PPSA` or `NPXS`)
    /// or explicitly declares native category parameters.
    #[must_use]
    pub fn is_ps5_native(&self) -> bool {
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
    /// The four measured off hardware, and no more. Anything a real title also carries is
    /// store metadata this project has no citable meaning for, and writing a guessed value
    /// into it would be worse than leaving it out.
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

    /// Configure a full native PS5 title descriptor.
    pub fn set_native_ps5(
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
    use super::Param;

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

    #[test]
    fn the_default_language_decides_the_title_and_not_the_first_locale() {
        // `ja-JP` comes first in the file. A reader that takes the first entry gets a
        // Japanese title for an English-default title: right shape, wrong answer, no error.
        let param = Param::parse(REAL_SHAPE.as_bytes()).expect("a document");
        assert_eq!(param.title_name(), Some("A Name"));
        assert_eq!(param.default_language(), Some("en-US"));
        assert_eq!(param.title_name_in("ja-JP"), Some("本当"));
    }

    #[test]
    fn a_file_without_a_default_still_names_its_title() {
        let param = Param::parse(br#"{"localizedParameters":{"fr-FR":{"titleName":"Un Nom"}}}"#)
            .expect("a document");
        assert_eq!(param.title_name(), Some("Un Nom"));
        assert_eq!(param.default_language(), None);
    }

    #[test]
    fn keys_this_module_does_not_name_survive_a_round_trip() {
        // Principle 4. A struct of known fields would drop `somethingUndocumented`, and the
        // file would come back shorter than it went in on every real title.
        let param = Param::parse(REAL_SHAPE.as_bytes()).expect("a document");
        let written = param.to_bytes().expect("bytes");
        let again = Param::parse(&written).expect("a document");
        assert_eq!(param, again);
        assert!(again.document().contains_key("somethingUndocumented"));
    }

    #[test]
    fn a_category_written_as_a_string_is_read_rather_than_dropped() {
        // Real dumping tools write both forms.
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

    #[test]
    fn languages_lists_only_locales_that_name_a_title() {
        // `defaultLanguage` is a sibling of the locale blocks, not one of them, and listing
        // it as a language would offer callers a locale that has no block.
        let param = Param::parse(REAL_SHAPE.as_bytes()).expect("a document");
        let mut languages = param.languages();
        languages.sort_unstable();
        assert_eq!(languages, ["en-US", "ja-JP"]);
    }

    #[test]
    fn json_that_is_not_an_object_is_refused() {
        assert!(Param::parse(b"[1,2,3]").is_err());
        assert!(Param::parse(b"not json").is_err());
    }

    #[test]
    fn native_ps5_title_configuration_and_detection() {
        let mut param = Param::new();
        param.set_native_ps5(
            "PPSA01650",
            "YouTube PS5",
            "en-US",
            0x10000,
            Some("UP0000-PPSA01650_00-YOUTUBE000000000"),
            Some("ps5://launch/youtube"),
        );
        param.set_version("01.00");
        param.set_master_version("01.00");
        param.set_sdk_version("12.40.00.01");

        assert!(param.is_ps5_native());
        assert_eq!(param.title_id(), Some("PPSA01650"));
        assert_eq!(
            param.content_id(),
            Some("UP0000-PPSA01650_00-YOUTUBE000000000")
        );
        assert_eq!(param.deeplink_uri(), Some("ps5://launch/youtube"));
        assert_eq!(param.version(), Some("01.00"));
        assert_eq!(param.master_version(), Some("01.00"));
        assert_eq!(param.sdk_version(), Some("12.40.00.01"));

        let bytes = param.to_bytes().expect("bytes");
        let parsed = Param::parse(&bytes).expect("document");
        assert_eq!(param, parsed);
        assert!(parsed.is_ps5_native());
    }
}
