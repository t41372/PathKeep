//! Menu bar menu text in English, Simplified and Traditional Chinese.
//!
//! The menu is drawn by the OS, not the webview, so its text cannot come from
//! the frontend catalog. This is the one place the backend writes UI copy;
//! the strings match the frontend's `shell.backup.*` and `common.*Ago` keys.
//!
//! ## Responsibilities
//! - Pick the menu language from the config preference and the OS languages.
//! - Produce the menu labels and the backup status line, and the macOS menu bar's labels.
//!
//! ## Not responsible for
//! - Deciding the backup status (`status.rs`) or building the menus (`tray.rs`,
//!   `app_menu.rs`).

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

/// The macOS menu bar's labels, in the words macOS itself uses for these items.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) struct AppMenuLabels {
    pub(crate) about: &'static str,
    pub(crate) services: &'static str,
    pub(crate) hide: &'static str,
    pub(crate) hide_others: &'static str,
    pub(crate) quit: &'static str,
    pub(crate) file: &'static str,
    pub(crate) close_window: &'static str,
    pub(crate) edit: &'static str,
    pub(crate) undo: &'static str,
    pub(crate) redo: &'static str,
    pub(crate) cut: &'static str,
    pub(crate) copy: &'static str,
    pub(crate) paste: &'static str,
    pub(crate) select_all: &'static str,
    pub(crate) view: &'static str,
    pub(crate) full_screen: &'static str,
    pub(crate) window: &'static str,
    pub(crate) minimize: &'static str,
    pub(crate) zoom: &'static str,
    pub(crate) help: &'static str,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn app_menu_labels(language: MenuLanguage) -> AppMenuLabels {
    let quit = menu_labels(language).quit;
    match language {
        MenuLanguage::En => AppMenuLabels {
            about: "About PathKeep",
            services: "Services",
            hide: "Hide PathKeep",
            hide_others: "Hide Others",
            quit,
            file: "File",
            close_window: "Close Window",
            edit: "Edit",
            undo: "Undo",
            redo: "Redo",
            cut: "Cut",
            copy: "Copy",
            paste: "Paste",
            select_all: "Select All",
            view: "View",
            full_screen: "Enter Full Screen",
            window: "Window",
            minimize: "Minimize",
            zoom: "Zoom",
            help: "Help",
        },
        MenuLanguage::ZhCn => AppMenuLabels {
            about: "关于 PathKeep",
            services: "服务",
            hide: "隐藏 PathKeep",
            hide_others: "隐藏其他",
            quit,
            file: "文件",
            close_window: "关闭窗口",
            edit: "编辑",
            undo: "撤销",
            redo: "重做",
            cut: "剪切",
            copy: "拷贝",
            paste: "粘贴",
            select_all: "全选",
            view: "显示",
            full_screen: "进入全屏幕",
            window: "窗口",
            minimize: "最小化",
            zoom: "缩放",
            help: "帮助",
        },
        MenuLanguage::ZhTw => AppMenuLabels {
            about: "關於 PathKeep",
            services: "服務",
            hide: "隱藏 PathKeep",
            hide_others: "隱藏其他",
            quit,
            file: "檔案",
            close_window: "關閉視窗",
            edit: "編輯",
            undo: "還原",
            redo: "重做",
            cut: "剪下",
            copy: "拷貝",
            paste: "貼上",
            select_all: "全選",
            view: "顯示方式",
            full_screen: "進入全螢幕",
            window: "視窗",
            minimize: "縮到最小",
            zoom: "縮放",
            help: "輔助說明",
        },
    }
}

/// What the first, disabled line of the menu says about backups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatusLine {
    BackingUp,
    BackedUp(DateTime<Utc>),
    Failed,
    Partial,
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
        (StatusLine::Partial, En) => "Backup incomplete".into(),
        (StatusLine::Partial, ZhCn) => "备份未完成".into(),
        (StatusLine::Partial, ZhTw) => "備份未完成".into(),
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
