//! OpenCC-compatible script folding for lexical recall.
//!
//! ## Responsibilities
//! - Load the small OpenCC dictionary subset needed by the accepted `t2s` and
//!   `tw2sp` search-normalization paths.
//! - Convert Traditional Chinese, Taiwan variants, and Taiwan idioms to a
//!   bounded set of Simplified Chinese recall forms before FTS indexing and
//!   keyword parsing.
//! - Keep the converter process-local and deterministic; dictionary parsing
//!   happens once per process through `LazyLock`.
//!
//! ## Not responsible for
//! - Shipping a general OpenCC replacement or command-line converter.
//! - Simplified-to-traditional, Hong Kong, Japanese, pinyin, or fuzzy recall.
//! - Loading dynamic libraries, SQLite extensions, CMake-built artifacts, or
//!   runtime dictionary files from the user machine.
//!
//! ## Dependencies
//! - Official OpenCC 1.3.0 Apache-2.0 dictionary assets vendored under
//!   `vendor/opencc`.
//! - Rust standard library collections and synchronization only.
//!
//! ## Performance notes
//! - Projection rebuilds may normalize millions of rows, so the dictionaries
//!   are parsed once and indexed by the first character of each key. Conversion
//!   checks only entries that can match the current character and applies
//!   longest-key priority with source-order tie-breaking.

use std::collections::HashMap;
use std::sync::LazyLock;

const TW_PHRASES_REV: &str = include_str!("../../vendor/opencc/dictionary/TWPhrasesRev.txt");
const TW_VARIANTS: &str = include_str!("../../vendor/opencc/dictionary/TWVariants.txt");
const TW_VARIANTS_REV_PHRASES: &str =
    include_str!("../../vendor/opencc/dictionary/TWVariantsRevPhrases.txt");
const TS_PHRASES: &str = include_str!("../../vendor/opencc/dictionary/TSPhrases.txt");
const TS_CHARACTERS: &str = include_str!("../../vendor/opencc/dictionary/TSCharacters.txt");

static SCRIPT_FOLDER: LazyLock<ScriptFolder> = LazyLock::new(ScriptFolder::from_opencc_assets);

/// Returns the OpenCC-derived Simplified Chinese forms used by lexical recall
/// so Traditional, Simplified, and Taiwan idiom variants can meet in the same
/// FTS projection. The first variant follows `t2s`; the optional second variant
/// follows `tw2sp` when Taiwan phrase folding differs.
pub(super) fn simplified_script_variants(input: &str) -> Vec<String> {
    SCRIPT_FOLDER.simplified_variants(input)
}

struct ScriptFolder {
    traditional_to_simplified: Dictionary,
    taiwan_to_opencc: Dictionary,
}

impl ScriptFolder {
    fn from_opencc_assets() -> Self {
        let mut taiwan = DictionaryBuilder::default();
        taiwan.add_forward_dictionary(TW_PHRASES_REV);
        taiwan.add_forward_dictionary(TW_VARIANTS_REV_PHRASES);
        taiwan.add_reversed_dictionary(TW_VARIANTS);

        let mut simplified = DictionaryBuilder::default();
        simplified.add_forward_dictionary(TS_PHRASES);
        simplified.add_forward_dictionary(TS_CHARACTERS);

        Self { traditional_to_simplified: simplified.build(), taiwan_to_opencc: taiwan.build() }
    }

    fn simplified_variants(&self, input: &str) -> Vec<String> {
        let direct = self.traditional_to_simplified.convert(input);
        let opencc_traditional = self.taiwan_to_opencc.convert(input);
        let regional = self.traditional_to_simplified.convert(&opencc_traditional);
        unique_variants([direct, regional])
    }
}

#[derive(Default)]
struct DictionaryBuilder {
    entries_by_first: HashMap<char, Vec<Entry>>,
    next_order: usize,
}

impl DictionaryBuilder {
    fn add_forward_dictionary(&mut self, raw: &'static str) {
        for (key, value) in dictionary_rows(raw) {
            self.insert(key, value);
        }
    }

    fn add_reversed_dictionary(&mut self, raw: &'static str) {
        for (key, value) in dictionary_rows(raw) {
            self.insert(value, key);
        }
    }

    fn insert(&mut self, key: &'static str, value: &'static str) {
        if key.is_empty() || value.is_empty() {
            return;
        }
        let key_chars = key.chars().collect::<Box<[_]>>();
        let first = key_chars[0];
        let entry = Entry { key: key_chars, value, order: self.next_order };
        self.next_order += 1;
        self.entries_by_first.entry(first).or_default().push(entry);
    }

    fn build(mut self) -> Dictionary {
        for entries in self.entries_by_first.values_mut() {
            entries.sort_by(|left, right| {
                right.key.len().cmp(&left.key.len()).then_with(|| left.order.cmp(&right.order))
            });
        }
        Dictionary { entries_by_first: self.entries_by_first }
    }
}

struct Dictionary {
    entries_by_first: HashMap<char, Vec<Entry>>,
}

impl Dictionary {
    fn convert(&self, input: &str) -> String {
        let input_chars = input.chars().collect::<Vec<_>>();
        let mut converted = String::with_capacity(input.len());
        let mut index = 0usize;

        while index < input_chars.len() {
            if let Some(entry) = self.longest_match_at(&input_chars, index) {
                converted.push_str(entry.value);
                index += entry.key.len();
            } else {
                converted.push(input_chars[index]);
                index += 1;
            }
        }

        converted
    }

    fn longest_match_at(&self, input: &[char], index: usize) -> Option<&Entry> {
        self.entries_by_first
            .get(&input[index])?
            .iter()
            .find(|entry| input.get(index..index + entry.key.len()) == Some(entry.key.as_ref()))
    }
}

struct Entry {
    key: Box<[char]>,
    value: &'static str,
    order: usize,
}

fn dictionary_rows(raw: &'static str) -> impl Iterator<Item = (&'static str, &'static str)> {
    raw.lines().filter_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        let (key, values) = trimmed.split_once('\t')?;
        let first_value = values.split_whitespace().next()?;
        Some((key, first_value))
    })
}

fn unique_variants(variants: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut unique = Vec::new();
    for variant in variants {
        if !unique.iter().any(|existing| existing == &variant) {
            unique.push(variant);
        }
    }
    unique
}
