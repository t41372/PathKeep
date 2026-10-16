//! Responsibilities: shared backend transport models, including semantic index control.
//! Not responsible for: queue policy, provider execution or persistence.
//!
//! These serde types are the contract between the desktop shell, worker layer,
//! and canonical backend modules. They should stay descriptive and transport
//! friendly rather than smuggling behavior.

pub mod annotations;
pub mod app;
pub mod archive;
pub mod audit;
pub mod core_intelligence;
pub mod import;
pub mod intelligence;
pub mod progress;
pub mod schedule;
pub mod security;
mod semantic_index;
pub mod stars;

pub use self::{
    annotations::*, app::*, archive::*, audit::*, core_intelligence::*, import::*, intelligence::*,
    progress::*, schedule::*, security::*, stars::*,
};
/// Re-exports the full backend model surface for the rest of the workspace.
pub use semantic_index::*;

#[cfg(test)]
mod tests {
    // ─── OgImage settings / fetch-mode coverage ──────────────────────

    #[test]
    fn og_image_settings_effective_mode_truth_table() {
        // (fetch_enabled, fetch_mode) → expected effective_mode
        let cases = [
            (true, super::OgImageFetchMode::Background, super::OgImageFetchMode::Background),
            (true, super::OgImageFetchMode::OnDemand, super::OgImageFetchMode::OnDemand),
            (true, super::OgImageFetchMode::Off, super::OgImageFetchMode::Off),
            // Kill switch wins regardless of mode.
            (false, super::OgImageFetchMode::Background, super::OgImageFetchMode::Off),
            (false, super::OgImageFetchMode::OnDemand, super::OgImageFetchMode::Off),
            (false, super::OgImageFetchMode::Off, super::OgImageFetchMode::Off),
        ];
        for (fetch_enabled, fetch_mode, expected) in cases {
            let settings = super::OgImageSettings {
                fetch_enabled,
                fetch_mode,
                ..super::OgImageSettings::default()
            };
            assert_eq!(
                settings.effective_mode(),
                expected,
                "fetch_enabled={fetch_enabled} fetch_mode={fetch_mode:?} should resolve to {expected:?}",
            );
        }
    }
}

mod intelligence_pages;
pub use intelligence_pages::*;
