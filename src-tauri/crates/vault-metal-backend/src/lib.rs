//! Metal-backend activation shim — intentionally code-free.
//!
//! Why this crate exists: it re-declares the candle trio (`candle-core`, `candle-nn`,
//! `candle-transformers`) with candle's `metal` feature so that vault-core can attach it as a
//! macOS-only, `optional` dependency behind vault-core's opt-in `metal` feature. Cargo unifies that
//! feature onto vault-core's own candle dependencies on Apple targets — turning on the real GPU
//! backend — without vault-core having to alias `candle-core` under a second name (which Cargo
//! rejects as "same package, two names"). On non-Apple targets the shim is absent from the build
//! graph, so candle's Metal path (candle-metal-kernels → objc2-metal → objc2, which
//! `compile_error!`s off Apple) is never reached, keeping `cargo clippy --all-features` green on the
//! Linux CI runner.
//!
//! Not responsible for: any runtime behaviour. Do not add code here.
