//! Tiny host-capability helpers shared by platform adapters.

#[cfg(target_os = "macos")]
fn compiled_platform_name() -> &'static str {
    "macos"
}

#[cfg(target_os = "windows")]
fn compiled_platform_name() -> &'static str {
    "windows"
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn compiled_platform_name() -> &'static str {
    "linux"
}

/// Returns the normalized platform name used across PathKeep read models.
pub fn current_platform_name() -> String {
    compiled_platform_name().to_string()
}

/// Whether this desktop session can show a menu bar / system tray icon.
///
/// macOS and Windows always can. On Linux the icon goes through
/// libappindicator to a StatusNotifier host in the panel; both are optional
/// (stock GNOME has no host without an extension), and the tray library
/// panics when it cannot load libappindicator, so check before offering it.
pub fn menu_bar_icon_supported() -> bool {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        true
    }
    #[cfg(target_os = "linux")]
    {
        static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *SUPPORTED.get_or_init(|| {
            linux_tray::appindicator_loadable()
                && linux_tray::status_notifier_host_present() != Some(false)
        })
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

#[cfg(target_os = "linux")]
mod linux_tray {
    use std::{ffi::CString, time::Duration};

    /// The libraries `libappindicator-sys` tries, in its order.
    const LIBRARIES: [&str; 4] = [
        "libayatana-appindicator3.so.1",
        "libappindicator3.so.1",
        "libayatana-appindicator3.so",
        "libappindicator3.so",
    ];

    pub(super) fn appindicator_loadable() -> bool {
        LIBRARIES.iter().any(|name| {
            let Ok(name) = CString::new(*name) else {
                return false;
            };
            // SAFETY: `name` is a valid C string; the handle is closed at once.
            let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) };
            if handle.is_null() {
                return false;
            }
            unsafe { libc::dlclose(handle) };
            true
        })
    }

    /// Whether a panel is listening for StatusNotifier icons; `None` when
    /// the session bus cannot be asked.
    pub(super) fn status_notifier_host_present() -> Option<bool> {
        let connection = dbus::blocking::Connection::new_session().ok()?;
        let proxy = connection.with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            Duration::from_millis(500),
        );
        let (owned,): (bool,) = proxy
            .method_call("org.freedesktop.DBus", "NameHasOwner", ("org.kde.StatusNotifierWatcher",))
            .ok()?;
        Some(owned)
    }
}

#[cfg(test)]
mod tests {
    use super::current_platform_name;

    #[test]
    fn current_platform_name_is_supported() {
        let value = current_platform_name();
        assert!(matches!(value.as_str(), "macos" | "windows" | "linux"));
    }
}
