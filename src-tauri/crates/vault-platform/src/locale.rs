//! The user's preferred UI languages, as the OS reports them.
//!
//! The webview resolves "System" language from `navigator.languages`. Native
//! surfaces drawn by Rust (the menu bar menu) need the same answer without a
//! webview, so this reads the OS setting directly.
//!
//! ## Responsibilities
//! - Return BCP 47-ish language tags, most preferred first.
//!
//! ## Not responsible for
//! - Choosing among PathKeep's supported languages (the caller does that).

/// Returns the OS's preferred UI language tags, most preferred first.
///
/// May be empty when the OS gives no answer; callers fall back to English.
pub fn preferred_ui_languages() -> Vec<String> {
    platform_languages()
}

#[cfg(target_os = "macos")]
fn platform_languages() -> Vec<String> {
    objc2_foundation::NSLocale::preferredLanguages().iter().map(|tag| tag.to_string()).collect()
}

#[cfg(target_os = "windows")]
fn platform_languages() -> Vec<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: takes no arguments and only returns the current user's LANGID.
    let langid = unsafe { GetUserDefaultUILanguage() };
    windows_langid_tag(langid).map(str::to_string).into_iter().collect()
}

/// Maps the Windows LANGIDs PathKeep cares about to a language tag.
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn windows_langid_tag(langid: u16) -> Option<&'static str> {
    const LANG_CHINESE: u16 = 0x04;
    const LANG_ENGLISH: u16 = 0x09;
    let primary = langid & 0x3ff;
    let sublanguage = langid >> 10;
    match primary {
        // Sub-languages 1 (Taiwan), 3 (Hong Kong), 5 (Macao) and 0x1f
        // (zh-Hant) are Traditional; 0 (zh-Hans), 2 (PRC) and 4 (Singapore)
        // are Simplified.
        LANG_CHINESE if matches!(sublanguage, 1 | 3 | 5 | 0x1f) => Some("zh-TW"),
        LANG_CHINESE => Some("zh-CN"),
        LANG_ENGLISH => Some("en"),
        _ => None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_languages() -> Vec<String> {
    let mut tags = Vec::new();
    // LANGUAGE is a colon-separated priority list; the others hold one locale.
    if let Ok(list) = std::env::var("LANGUAGE") {
        tags.extend(list.split(':').filter(|tag| !tag.is_empty()).map(str::to_string));
    }
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name)
            && !value.is_empty()
        {
            tags.push(value);
        }
    }
    // "zh_TW.UTF-8" → "zh-TW"
    tags.into_iter()
        .map(|tag| tag.split('.').next().unwrap_or_default().replace('_', "-"))
        .filter(|tag| !tag.is_empty() && tag != "C" && tag != "POSIX")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_langids_map_to_supported_languages() {
        assert_eq!(windows_langid_tag(0x0404), Some("zh-TW"));
        assert_eq!(windows_langid_tag(0x0c04), Some("zh-TW"));
        assert_eq!(windows_langid_tag(0x1404), Some("zh-TW"));
        assert_eq!(windows_langid_tag(0x7c04), Some("zh-TW"));
        assert_eq!(windows_langid_tag(0x0804), Some("zh-CN"));
        assert_eq!(windows_langid_tag(0x1004), Some("zh-CN"));
        assert_eq!(windows_langid_tag(0x0409), Some("en"));
        assert_eq!(windows_langid_tag(0x0809), Some("en"));
        assert_eq!(windows_langid_tag(0x0411), None);
    }

    #[test]
    fn preferred_ui_languages_returns_tags_without_encodings() {
        for tag in preferred_ui_languages() {
            assert!(!tag.is_empty());
            assert!(!tag.contains('.'), "{tag}");
        }
    }
}
