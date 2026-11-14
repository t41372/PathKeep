//! Responsibilities: isolate real owner sessions and observe files without a production inventory.
//! Not responsible for: choosing expected members or making lifecycle assertions for scenarios.

// Preserve the independently handed-over source byte for byte, including its own tests.
#[rustfmt::skip]
pub mod expected_root;
mod fixture;
mod names;
mod observation;
mod zip;
pub use fixture::*;
pub use observation::*;
pub use zip::*;
