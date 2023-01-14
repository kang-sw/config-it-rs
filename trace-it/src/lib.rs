use log::LevelFilter;

mod builder;
mod detail;

pub use builder::Builder;

///
///
/// Trace subscriber provides support for tracing events and metrics. To make this cover all
/// log outputs from the system, it is recommended to use `tracing_log` crate.
///
pub(crate) struct TraceSubscriber {}
