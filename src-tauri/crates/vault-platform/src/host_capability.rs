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

pub fn current_platform_name() -> String {
    compiled_platform_name().to_string()
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
