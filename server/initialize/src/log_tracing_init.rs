use tracing_log::LogTracer;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

use crate::{project_error, project_info};

pub async fn initialize_log_tracing() {
    if let Err(e) = LogTracer::init() {
        project_error!("Failed to set logger: {}", e);
        return;
    }

    let env_filter = if cfg!(debug_assertions) {
        EnvFilter::new("debug,sea_orm=debug")
    } else {
        EnvFilter::new("info,sea_orm=info")
    };

    // 044 W-F12 (FR-002, DESIGN-W §8.1): JSON-formatted log rows for loki/promtail.
    // Output schema: { timestamp, level, target, span: {leaf}, spans: [http_req,...], fields: { message, ... } }
    // - `.json()` switches the formatter from human-readable to JSON
    // - `with_current_span(true)` includes the immediate (leaf) span's fields under `span`
    // - `with_span_list(true)` includes the full ancestor span chain under `spans` — the
    //   `http_req` span (with `service`/`request_id`/`method`/`route`) is in the chain so
    //   request_id is reachable from any log row emitted inside an HTTP request, regardless
    //   of how many nested macro-spans wrap it. promtail pipeline_stages parses `spans[*].request_id`.
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_target(true)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .with(tracing_error::ErrorLayer::default());

    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        project_error!("Failed to set subscriber: {}", e);
        return;
    }

    if cfg!(debug_assertions) {
        project_info!("Log tracing initialized successfully in debug mode");
    } else {
        project_info!("Log tracing initialized successfully in release mode");
    }
}
