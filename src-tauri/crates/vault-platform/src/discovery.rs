use anyhow::Result;

pub fn discover_browser_profiles() -> Result<Vec<vault_core::BrowserProfile>> {
    vault_core::discover_profiles()
}
