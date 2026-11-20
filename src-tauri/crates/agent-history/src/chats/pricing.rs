//! Standard text-token price estimates in USD. Native charges take precedence.
//! Unknown model IDs and unsupported cache tiers remain unpriced.
use super::facts::Tokens;
pub(crate) const VERSION: &str = "standard-text-2026-11-20";
pub(super) fn estimate(model: Option<&str>, t: Tokens, hour_write: i64) -> Option<f64> {
    let (input, write, read, output): (f64, f64, f64, f64) = match model? {
        "claude-sonnet-4-6" | "claude-sonnet-4-5" | "claude-sonnet-4-5-20250929" => {
            (3., 3.75, 0.30, 15.)
        }
        "claude-opus-4-6" | "claude-opus-4-5" | "claude-opus-4-5-20251101" => (5., 6.25, 0.50, 25.),
        "claude-haiku-4-5" | "claude-haiku-4-5-20251001" => (1., 1.25, 0.10, 5.),
        "gpt-5.1" | "gpt-5.1-2025-11-13" | "gpt-5.1-codex" | "gpt-5.1-codex-max"
            if t.cache_write == 0 =>
        {
            (1.25, 0., 0.125, 10.)
        }
        _ => return None,
    };
    Some(
        (t.input as f64 * input
            + (t.cache_write - hour_write) as f64 * write
            + hour_write as f64 * input * 2.
            + t.cache_read as f64 * read
            + t.output as f64 * output)
            / 1_000_000.,
    )
}
