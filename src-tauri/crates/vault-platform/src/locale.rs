//! The user's preferred UI languages, as the OS reports them.
//!
//! The webview resolves "System" language from `navigator.languages`. Native
//! surfaces drawn by Rust (the menu bar menu) need the same answer without a
//! webview, so this reads the OS setting directly.
//!
//! ## Responsibilities
//! - Return BCP 47-ish language tags, most preferred first.
//! - Say whether the OS shows times on a 12- or a 24-hour clock, which the webview's `Intl`
//!   cannot tell: it follows the language, not the user's clock setting.
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
#[cfg(target_os = "windows")]
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

/// Whether the OS shows times on a 12-hour clock (`Some(true)`) or a 24-hour one
/// (`Some(false)`); `None` when it does not say, and the language's own default applies.
pub fn uses_12_hour_clock() -> Option<bool> {
    platform_hour_pattern().map(|pattern| hour_pattern_is_12_hour(&pattern))
}

/// The same answer as the frontend's hour cycle names it: "h12", "h23", or none.
pub fn system_hour_cycle() -> Option<&'static str> {
    uses_12_hour_clock().map(|twelve| if twelve { "h12" } else { "h23" })
}

/// macOS: the hour field the current locale prefers, with the "24-hour time" switch folded in.
#[cfg(target_os = "macos")]
fn platform_hour_pattern() -> Option<String> {
    use objc2_foundation::{NSDateFormatter, NSLocale, NSString};
    let locale = NSLocale::currentLocale();
    NSDateFormatter::dateFormatFromTemplate_options_locale(
        &NSString::from_str("j"),
        0,
        Some(&*locale),
    )
    .map(|pattern| pattern.to_string())
}

/// Windows: the user's short time format (Settings → Time & language → Region).
#[cfg(target_os = "windows")]
fn platform_hour_pattern() -> Option<String> {
    const LOCALE_SSHORTTIME: u32 = 0x79;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocaleInfoEx(name: *const u16, lctype: u32, data: *mut u16, len: i32) -> i32;
    }
    let mut buffer = [0u16; 80];
    // SAFETY: a null name is LOCALE_NAME_USER_DEFAULT; the buffer length is passed in UTF-16
    // units and the call writes at most that many, returning 0 on failure.
    let written = unsafe {
        GetLocaleInfoEx(
            std::ptr::null(),
            LOCALE_SSHORTTIME,
            buffer.as_mut_ptr(),
            buffer.len() as i32,
        )
    };
    (written > 1).then(|| String::from_utf16_lossy(&buffer[..written as usize - 1]))
}

/// Linux has no clock setting a desktop app can read.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_hour_pattern() -> Option<String> {
    None
}

/// An LDML or Windows time pattern uses `h` / `K` for a 12-hour clock and `H` / `k` for a
/// 24-hour one; text in single quotes is literal.
fn hour_pattern_is_12_hour(pattern: &str) -> bool {
    let mut quoted = false;
    for ch in pattern.chars() {
        match ch {
            '\'' => quoted = !quoted,
            'h' | 'K' if !quoted => return true,
            'H' | 'k' if !quoted => return false,
            _ => {}
        }
    }
    false
}
