//! Menu bar menu text in English, Simplified and Traditional Chinese.
//!
//! The menu is drawn by the OS, not the webview, so its text cannot come from
//! the frontend catalog. This is the one place the backend writes UI copy;
//! the strings match the frontend's `shell.backup.*` and `common.*Ago` keys.
//!
//! ## Responsibilities
//! - Pick the menu language from the config preference and the OS languages.
//! - Produce the menu labels and the backup status line.
//!
//! ## Not responsible for
//! - Deciding the backup status (`status.rs`) or building the menu (`tray.rs`).

use chrono::{DateTime, Utc};
use vault_core::LanguagePreference;

/// Language of the menu bar menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MenuLanguage {
    #[default]
    En,
    ZhCn,
    ZhTw,
}

impl MenuLanguage {
    /// Uses the configured language, or for "System" the first OS language
    /// PathKeep supports — the same rule as the frontend's
    /// `detectSystemLanguage`, so the menu and the window agree.
    pub(crate) fn resolve(preference: &LanguagePreference, os_languages: &[String]) -> Self {
        match preference {
            LanguagePreference::En => Self::En,
            LanguagePreference::ZhCn => Self::ZhCn,
            LanguagePreference::ZhTw => Self::ZhTw,
            LanguagePreference::System => os_languages
                .iter()
                .find_map(|tag| {
                    let tag = tag.to_ascii_lowercase();
                    if tag.starts_with("zh") {
                        let traditional =
                            ["hant", "tw", "hk", "mo"].iter().any(|marker| tag.contains(marker));
                        Some(if traditional { Self::ZhTw } else { Self::ZhCn })
                    } else if tag.starts_with("en") {
                        Some(Self::En)
                    } else {
                        None
                    }
                })
                .unwrap_or_default(),
        }
    }
}

/// Fixed menu labels.
pub(crate) struct MenuLabels {
    pub(crate) back_up_now: &'static str,
    pub(crate) search_history: &'static str,
    pub(crate) open: &'static str,
    pub(crate) quit: &'static str,
}

pub(crate) fn menu_labels(language: MenuLanguage) -> MenuLabels {
    match language {
        MenuLanguage::En => MenuLabels {
            back_up_now: "Back up now",
            search_history: "Search history…",
            open: "Open PathKeep",
            quit: "Quit PathKeep",
        },
        MenuLanguage::ZhCn => MenuLabels {
            back_up_now: "立即备份",
            search_history: "搜索历史…",
            open: "打开 PathKeep",
            quit: "退出 PathKeep",
        },
        MenuLanguage::ZhTw => MenuLabels {
            back_up_now: "立即備份",
            search_history: "搜尋歷史…",
            open: "打開 PathKeep",
            quit: "結束 PathKeep",
        },
    }
}

/// What the first, disabled line of the menu says about backups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatusLine {
    BackingUp,
    BackedUp(DateTime<Utc>),
    Failed,
    NeverBackedUp,
    NotSetUp,
    /// The archive could not be read (for example, it is locked) and no
    /// scheduled backup has recorded a success.
    Unknown,
}

pub(crate) fn status_text(language: MenuLanguage, line: &StatusLine, now: DateTime<Utc>) -> String {
    use MenuLanguage::*;
    match (line, language) {
        (StatusLine::BackingUp, En) => "Backing up…".into(),
        (StatusLine::BackingUp, ZhCn) => "备份中…".into(),
        (StatusLine::BackingUp, ZhTw) => "備份中…".into(),
        (StatusLine::BackedUp(at), En) => format!("Backed up {}", ago(language, *at, now)),
        (StatusLine::BackedUp(at), ZhCn) => format!("{}已备份", ago(language, *at, now)),
        (StatusLine::BackedUp(at), ZhTw) => format!("{}已備份", ago(language, *at, now)),
        (StatusLine::Failed, En) => "Backup failed".into(),
        (StatusLine::Failed, ZhCn) => "备份失败".into(),
        (StatusLine::Failed, ZhTw) => "備份失敗".into(),
        (StatusLine::NeverBackedUp, En) => "Not backed up yet".into(),
        (StatusLine::NeverBackedUp, ZhCn) => "还没有备份过".into(),
        (StatusLine::NeverBackedUp, ZhTw) => "還沒有備份過".into(),
        (StatusLine::NotSetUp, En) => "PathKeep isn’t set up yet".into(),
        (StatusLine::NotSetUp, ZhCn) => "PathKeep 还没有设置".into(),
        (StatusLine::NotSetUp, ZhTw) => "PathKeep 尚未設定".into(),
        (StatusLine::Unknown, En) => "Backup status unavailable".into(),
        (StatusLine::Unknown, ZhCn) => "暂时无法读取备份状态".into(),
        (StatusLine::Unknown, ZhTw) => "暫時無法讀取備份狀態".into(),
    }
}

/// "12 min ago" / "12 分钟前", matching the frontend's `common.*Ago` keys.
fn ago(language: MenuLanguage, at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let minutes = (now - at).num_minutes().max(0);
    let (count, unit) = match minutes {
        0 => {
            return match language {
                MenuLanguage::En => "just now",
                MenuLanguage::ZhCn => "刚刚",
                MenuLanguage::ZhTw => "剛剛",
            }
            .to_string();
        }
        1..=59 => (minutes, Unit::Minute),
        60..=1439 => (minutes / 60, Unit::Hour),
        _ => (minutes / 1440, Unit::Day),
    };
    match (language, unit) {
        (MenuLanguage::En, Unit::Minute) => format!("{count} min ago"),
        (MenuLanguage::En, Unit::Hour) if count == 1 => "1 hour ago".into(),
        (MenuLanguage::En, Unit::Hour) => format!("{count} hours ago"),
        (MenuLanguage::En, Unit::Day) if count == 1 => "1 day ago".into(),
        (MenuLanguage::En, Unit::Day) => format!("{count} days ago"),
        (MenuLanguage::ZhCn, Unit::Minute) => format!("{count} 分钟前"),
        (MenuLanguage::ZhCn, Unit::Hour) => format!("{count} 小时前"),
        (MenuLanguage::ZhTw, Unit::Minute) => format!("{count} 分鐘前"),
        (MenuLanguage::ZhTw, Unit::Hour) => format!("{count} 小時前"),
        (_, Unit::Day) => format!("{count} 天前"),
    }
}

enum Unit {
    Minute,
    Hour,
    Day,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn tags(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn explicit_preference_wins_over_the_os() {
        let os = tags(&["zh-Hant-TW"]);
        assert_eq!(MenuLanguage::resolve(&LanguagePreference::En, &os), MenuLanguage::En);
        assert_eq!(MenuLanguage::resolve(&LanguagePreference::ZhCn, &os), MenuLanguage::ZhCn);
    }

    #[test]
    fn system_preference_takes_the_first_supported_os_language() {
        let system = LanguagePreference::System;
        let resolve = |values: &[&str]| MenuLanguage::resolve(&system, &tags(values));
        assert_eq!(resolve(&["ja-JP", "zh-Hant-TW", "en"]), MenuLanguage::ZhTw);
        assert_eq!(resolve(&["zh-Hans-CN"]), MenuLanguage::ZhCn);
        assert_eq!(resolve(&["zh-HK"]), MenuLanguage::ZhTw);
        assert_eq!(resolve(&["en-GB", "zh-TW"]), MenuLanguage::En);
        assert_eq!(resolve(&["fr-FR"]), MenuLanguage::En);
        assert_eq!(resolve(&[]), MenuLanguage::En);
    }

    #[test]
    fn status_line_reads_naturally_in_each_language() {
        let now = Utc::now();
        let line = |minutes| StatusLine::BackedUp(now - Duration::minutes(minutes));
        assert_eq!(status_text(MenuLanguage::En, &line(0), now), "Backed up just now");
        assert_eq!(status_text(MenuLanguage::En, &line(12), now), "Backed up 12 min ago");
        assert_eq!(status_text(MenuLanguage::En, &line(60), now), "Backed up 1 hour ago");
        assert_eq!(status_text(MenuLanguage::En, &line(150), now), "Backed up 2 hours ago");
        assert_eq!(status_text(MenuLanguage::En, &line(3 * 1440), now), "Backed up 3 days ago");
        assert_eq!(status_text(MenuLanguage::ZhCn, &line(12), now), "12 分钟前已备份");
        assert_eq!(status_text(MenuLanguage::ZhTw, &line(0), now), "剛剛已備份");
        assert_eq!(status_text(MenuLanguage::ZhTw, &line(1440), now), "1 天前已備份");
        // A clock that moved backwards must not print "-3 min ago".
        let future = StatusLine::BackedUp(now + Duration::minutes(3));
        assert_eq!(status_text(MenuLanguage::En, &future, now), "Backed up just now");
        assert_eq!(status_text(MenuLanguage::ZhCn, &StatusLine::BackingUp, now), "备份中…");
    }
}
