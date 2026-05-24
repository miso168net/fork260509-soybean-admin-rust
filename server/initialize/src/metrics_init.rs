// 044 W-F13: Prometheus metrics recorder + /metrics endpoint.
//
// Installs the global metrics recorder (PrometheusBuilder), pre-declares the 8
// metric names from data-model.md §E3 so prometheus / grafana dashboards do not
// show "No data" before any traffic, and exposes a small axum Router with
// `GET /metrics` returning the prometheus text exposition format.
//
// 8 metrics pre-declared (per data-model.md §E3):
//   counters (5): audit_log_writes_total / casbin_enforcement_total
//                 casbin_policy_cache_invalidate_total / cleanup_job_rows_deleted_total
//                 backup_completed_total
//   gauges   (2): outbox_pending_events / sys_tokens_active
//   histogram(1): http_request_duration_seconds
//
// Instrumentation of these series happens in Phase 4 (T020-T038); this module
// only registers the names so the series exist as 0-value before traffic.

use axum::{routing::get, Router};
use metrics::{
    counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram,
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

use crate::project_info;

pub fn init() -> Router {
    let handle: PrometheusHandle = PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install prometheus metric recorder");

    // ---- 5 counters ----
    describe_counter!(
        "audit_log_writes_total",
        "audit log row writes (INTERNAL + HTTP 雙視角)"
    );
    describe_counter!(
        "casbin_enforcement_total",
        "casbin enforce calls, label result=allow|deny"
    );
    describe_counter!(
        "casbin_policy_cache_invalidate_total",
        "casbin policy cache invalidate notifications"
    );
    describe_counter!(
        "cleanup_job_rows_deleted_total",
        "cleanup job rows deleted, label table_name"
    );
    describe_counter!(
        "backup_completed_total",
        "backup job completions (declared 0 series — W-F15/16 future instrument)"
    );
    // initialize counters at 0 so prometheus sees the series before traffic
    counter!("audit_log_writes_total").absolute(0);
    counter!("casbin_enforcement_total").absolute(0);
    counter!("casbin_policy_cache_invalidate_total").absolute(0);
    counter!("cleanup_job_rows_deleted_total").absolute(0);
    counter!("backup_completed_total").absolute(0);

    // ---- 2 gauges ----
    describe_gauge!(
        "outbox_pending_events",
        "audit outbox pending event queue depth"
    );
    describe_gauge!(
        "sys_tokens_active",
        "active sys_tokens count (declared 0 series — implementer follow-up)"
    );
    gauge!("outbox_pending_events").set(0.0);
    gauge!("sys_tokens_active").set(0.0);

    // ---- 1 histogram ----
    describe_histogram!(
        "http_request_duration_seconds",
        "HTTP request duration in seconds"
    );
    histogram!("http_request_duration_seconds").record(0.0);

    project_info!("Prometheus metrics recorder installed; 8 metric pre-declared");

    Router::new().route(
        "/metrics",
        get(move || {
            let h = handle.clone();
            async move { h.render() }
        }),
    )
}
