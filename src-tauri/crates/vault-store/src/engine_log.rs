//! Responsibilities: route the storage engine's `tracing` diagnostics into the app's `log` sink.
//! Not responsible for: the app's own log levels, files or rotation.
//! Failure modes: Turso's per-page spans reach the app logger (each B-tree step then pays the
//! logger's per-module filter, which made Insights rebuilds and regex scans several times slower
//! inside the app than in tests); engine warnings and errors are lost. Tests: tests/engine_log.rs.
//!
//! Turso instruments its B-tree and VDBE with `tracing` spans. With no `tracing` dispatcher set,
//! `tracing` falls back to `log` for every span it opens, enters, records and drops, so a process
//! with a `log` logger at Debug asks that logger about each one. A dispatcher that wants only
//! warnings and errors turns those spans off at their callsites, where the check is cached.
use tracing::{
    Event, Level, Metadata, Subscriber,
    field::{Field, Visit},
    level_filters::LevelFilter,
    span,
    subscriber::Interest,
};

/// Installs the bridge as the process's `tracing` dispatcher. Call once at startup, before the
/// store opens; a later call, or one after another dispatcher was set, changes nothing.
pub fn route_engine_diagnostics_to_log() {
    let _ = tracing::dispatcher::set_global_default(tracing::Dispatch::new(EngineLog));
}

struct EngineLog;

impl EngineLog {
    fn wanted(metadata: &Metadata<'_>) -> bool {
        metadata.is_event() && *metadata.level() <= Level::WARN
    }
}

impl Subscriber for EngineLog {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if Self::wanted(metadata) { Interest::always() } else { Interest::never() }
    }
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        Self::wanted(metadata)
    }
    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(LevelFilter::WARN)
    }
    // No span is ever enabled, so these are never asked for one.
    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }
    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
    fn enter(&self, _: &span::Id) {}
    fn exit(&self, _: &span::Id) {}
    fn event(&self, event: &Event<'_>) {
        let metadata = event.metadata();
        let level =
            if *metadata.level() == Level::ERROR { log::Level::Error } else { log::Level::Warn };
        let mut text = Text::default();
        event.record(&mut text);
        let text = text.message + &text.fields;
        log::logger().log(
            &log::Record::builder()
                .level(level)
                .target(metadata.target())
                .module_path(metadata.module_path())
                .file(metadata.file())
                .line(metadata.line())
                .args(format_args!("{text}"))
                .build(),
        );
    }
}

/// An event's message, then its other fields as ` name=value`.
#[derive(Default)]
struct Text {
    message: String,
    fields: String,
}

impl Visit for Text {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        let _ = if field.name() == "message" {
            write!(self.message, "{value:?}")
        } else {
            write!(self.fields, " {}={value:?}", field.name())
        };
    }
}
