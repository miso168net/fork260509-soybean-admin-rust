//! F12 cleanup binary — physically deletes expired soft-deleted rows.
//!
//! Usage:
//!   cleanup             # dry-run (default): prints what WOULD be deleted, exits 0
//!   cleanup --execute   # execute: deletes rows, writes HARD_DELETE audit, exits 0/non-zero
//!
//! Env vars:
//!   DATABASE_URL              (required) postgres connection string
//!   CLEANUP_RETENTION_DAYS    (optional, default 90) positive integer

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use metrics_exporter_prometheus::PrometheusBuilder;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use server_core::web::audit::{Actor, AuditEvent, AuditOperation, AuditSource};
use server_model::admin::{
    audit_log,
    audit_serialize::audit_snapshot,
    entities::{
        sys_access_key, sys_domain, sys_endpoint, sys_menu, sys_organization, sys_role, sys_user,
    },
};
use tracing::{error, info, warn};

// =============================================================================
// Pure functions (unit-tested below)
// =============================================================================

/// Parse `CLEANUP_RETENTION_DAYS` value.
/// - `None` (env var unset) → `Ok(90)` (default)
/// - empty string, `"0"`, negative string, non-numeric → `Err(&'static str)`
/// - positive integer string → `Ok(days)`
pub fn parse_retention_days(raw: Option<&str>) -> Result<u32, &'static str> {
    match raw {
        None => Ok(90),
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Err("CLEANUP_RETENTION_DAYS must not be empty");
            }
            match trimmed.parse::<i64>() {
                Ok(n) if n > 0 => Ok(n as u32),
                Ok(_) => Err("CLEANUP_RETENTION_DAYS must be a positive integer (> 0)"),
                Err(_) => Err("CLEANUP_RETENTION_DAYS is not a valid integer"),
            }
        }
    }
}

/// Compute the cutoff timestamp: rows deleted before this instant are "expired".
pub fn compute_cutoff(retention_days: u32, now: DateTime<Utc>) -> NaiveDateTime {
    (now - Duration::days(retention_days as i64)).naive_utc()
}

/// Format a dry-run report line for one table.
pub fn dry_run_line(table: &str, count: usize, cutoff: &NaiveDateTime) -> String {
    format!(
        "{}: {} row(s) will be deleted (deleted_at < {})",
        table, count, cutoff
    )
}

// =============================================================================
// Per-table sweep helpers — 7 separate blocks (PK types differ: i32 vs String)
// =============================================================================

/// Result of sweeping one table: (rows_deleted_ok, rows_failed)
struct SweepResult {
    ok: usize,
    failed: usize,
}

async fn sweep_sys_user(
    db: &DatabaseConnection,
    actor: &Actor,
    cutoff: &NaiveDateTime,
    execute: bool,
) -> SweepResult {
    let table = "sys_user";
    let rows = sys_user::Entity::find()
        .filter(sys_user::Column::DeletedAt.is_not_null())
        .filter(sys_user::Column::DeletedAt.lt(*cutoff))
        .all(db)
        .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            error!("{}: SELECT failed: {}", table, e);
            return SweepResult { ok: 0, failed: 1 };
        }
    };

    if !execute {
        info!("{}", dry_run_line(table, rows.len(), cutoff));
        // In dry-run mode `ok` carries "rows that would be deleted", not rows actually deleted.
        return SweepResult { ok: rows.len(), failed: 0 };
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    for model in rows {
        let id = model.id.clone();
        let before = audit_snapshot(&model);
        match db.begin().await {
            Err(e) => { error!("{} id={}: begin txn failed: {}", table, id, e); failed += 1; }
            Ok(txn) => {
                let audit_res = audit_log::write_in_txn(&txn, AuditEvent {
                    actor,
                    operation: AuditOperation::HardDelete,
                    entity_type: "sys_user",
                    entity_id: id.clone(),
                    payload_before: Some(before),
                    payload_after: None,
                    description: None,
                    source: AuditSource::Cleanup,
                    request_id: None,
                }).await;
                if let Err(e) = audit_res {
                    error!("{} id={}: audit insert failed: {}", table, id, e.message);
                    let _ = txn.rollback().await;
                    failed += 1;
                    continue;
                }
                let del_res = sys_user::Entity::delete_many()
                    .filter(sys_user::Column::Id.eq(&id))
                    .exec(&txn)
                    .await;
                match del_res {
                    Err(e) => {
                        error!("{} id={}: DELETE failed: {}", table, id, e);
                        let _ = txn.rollback().await;
                        failed += 1;
                    }
                    Ok(result) => {
                        if result.rows_affected == 0 {
                            warn!("{} id={}: DELETE affected 0 rows (already gone?), skipping", table, id);
                            let _ = txn.rollback().await;
                            failed += 1;
                        } else {
                            match txn.commit().await {
                                Err(e) => { error!("{} id={}: commit failed: {}", table, id, e); failed += 1; }
                                Ok(_) => { info!("{} id={}: hard-deleted", table, id); ok += 1; }
                            }
                        }
                    }
                }
            }
        }
    }
    if execute && ok > 0 {
        metrics::counter!(
            "cleanup_job_rows_deleted_total",
            "table_name" => table
        )
        .increment(ok as u64);
    }
    SweepResult { ok, failed }
}

async fn sweep_sys_role(
    db: &DatabaseConnection,
    actor: &Actor,
    cutoff: &NaiveDateTime,
    execute: bool,
) -> SweepResult {
    let table = "sys_role";
    let rows = sys_role::Entity::find()
        .filter(sys_role::Column::DeletedAt.is_not_null())
        .filter(sys_role::Column::DeletedAt.lt(*cutoff))
        .all(db)
        .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => { error!("{}: SELECT failed: {}", table, e); return SweepResult { ok: 0, failed: 1 }; }
    };

    if !execute {
        info!("{}", dry_run_line(table, rows.len(), cutoff));
        // In dry-run mode `ok` carries "rows that would be deleted", not rows actually deleted.
        return SweepResult { ok: rows.len(), failed: 0 };
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    for model in rows {
        let id = model.id.clone();
        let before = audit_snapshot(&model);
        match db.begin().await {
            Err(e) => { error!("{} id={}: begin txn failed: {}", table, id, e); failed += 1; }
            Ok(txn) => {
                let audit_res = audit_log::write_in_txn(&txn, AuditEvent {
                    actor,
                    operation: AuditOperation::HardDelete,
                    entity_type: "sys_role",
                    entity_id: id.clone(),
                    payload_before: Some(before),
                    payload_after: None,
                    description: None,
                    source: AuditSource::Cleanup,
                    request_id: None,
                }).await;
                if let Err(e) = audit_res {
                    error!("{} id={}: audit insert failed: {}", table, id, e.message);
                    let _ = txn.rollback().await;
                    failed += 1;
                    continue;
                }
                let del_res = sys_role::Entity::delete_many()
                    .filter(sys_role::Column::Id.eq(&id))
                    .exec(&txn)
                    .await;
                match del_res {
                    Err(e) => { error!("{} id={}: DELETE failed: {}", table, id, e); let _ = txn.rollback().await; failed += 1; }
                    Ok(result) => {
                        if result.rows_affected == 0 {
                            warn!("{} id={}: DELETE affected 0 rows (already gone?), skipping", table, id);
                            let _ = txn.rollback().await;
                            failed += 1;
                        } else {
                            match txn.commit().await {
                                Err(e) => { error!("{} id={}: commit failed: {}", table, id, e); failed += 1; }
                                Ok(_) => { info!("{} id={}: hard-deleted", table, id); ok += 1; }
                            }
                        }
                    }
                }
            }
        }
    }
    if execute && ok > 0 {
        metrics::counter!(
            "cleanup_job_rows_deleted_total",
            "table_name" => table
        )
        .increment(ok as u64);
    }
    SweepResult { ok, failed }
}

async fn sweep_sys_menu(
    db: &DatabaseConnection,
    actor: &Actor,
    cutoff: &NaiveDateTime,
    execute: bool,
) -> SweepResult {
    let table = "sys_menu";
    let rows = sys_menu::Entity::find()
        .filter(sys_menu::Column::DeletedAt.is_not_null())
        .filter(sys_menu::Column::DeletedAt.lt(*cutoff))
        .all(db)
        .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => { error!("{}: SELECT failed: {}", table, e); return SweepResult { ok: 0, failed: 1 }; }
    };

    if !execute {
        info!("{}", dry_run_line(table, rows.len(), cutoff));
        // In dry-run mode `ok` carries "rows that would be deleted", not rows actually deleted.
        return SweepResult { ok: rows.len(), failed: 0 };
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    for model in rows {
        let id = model.id; // i32
        let before = audit_snapshot(&model);
        match db.begin().await {
            Err(e) => { error!("{} id={}: begin txn failed: {}", table, id, e); failed += 1; }
            Ok(txn) => {
                let audit_res = audit_log::write_in_txn(&txn, AuditEvent {
                    actor,
                    operation: AuditOperation::HardDelete,
                    entity_type: "sys_menu",
                    entity_id: id.to_string(),
                    payload_before: Some(before),
                    payload_after: None,
                    description: None,
                    source: AuditSource::Cleanup,
                    request_id: None,
                }).await;
                if let Err(e) = audit_res {
                    error!("{} id={}: audit insert failed: {}", table, id, e.message);
                    let _ = txn.rollback().await;
                    failed += 1;
                    continue;
                }
                let del_res = sys_menu::Entity::delete_many()
                    .filter(sys_menu::Column::Id.eq(id))
                    .exec(&txn)
                    .await;
                match del_res {
                    Err(e) => { error!("{} id={}: DELETE failed: {}", table, id, e); let _ = txn.rollback().await; failed += 1; }
                    Ok(result) => {
                        if result.rows_affected == 0 {
                            warn!("{} id={}: DELETE affected 0 rows (already gone?), skipping", table, id);
                            let _ = txn.rollback().await;
                            failed += 1;
                        } else {
                            match txn.commit().await {
                                Err(e) => { error!("{} id={}: commit failed: {}", table, id, e); failed += 1; }
                                Ok(_) => { info!("{} id={}: hard-deleted", table, id); ok += 1; }
                            }
                        }
                    }
                }
            }
        }
    }
    if execute && ok > 0 {
        metrics::counter!(
            "cleanup_job_rows_deleted_total",
            "table_name" => table
        )
        .increment(ok as u64);
    }
    SweepResult { ok, failed }
}

async fn sweep_sys_domain(
    db: &DatabaseConnection,
    actor: &Actor,
    cutoff: &NaiveDateTime,
    execute: bool,
) -> SweepResult {
    let table = "sys_domain";
    let rows = sys_domain::Entity::find()
        .filter(sys_domain::Column::DeletedAt.is_not_null())
        .filter(sys_domain::Column::DeletedAt.lt(*cutoff))
        .all(db)
        .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => { error!("{}: SELECT failed: {}", table, e); return SweepResult { ok: 0, failed: 1 }; }
    };

    if !execute {
        info!("{}", dry_run_line(table, rows.len(), cutoff));
        // In dry-run mode `ok` carries "rows that would be deleted", not rows actually deleted.
        return SweepResult { ok: rows.len(), failed: 0 };
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    for model in rows {
        let id = model.id.clone();
        let before = audit_snapshot(&model);
        match db.begin().await {
            Err(e) => { error!("{} id={}: begin txn failed: {}", table, id, e); failed += 1; }
            Ok(txn) => {
                let audit_res = audit_log::write_in_txn(&txn, AuditEvent {
                    actor,
                    operation: AuditOperation::HardDelete,
                    entity_type: "sys_domain",
                    entity_id: id.clone(),
                    payload_before: Some(before),
                    payload_after: None,
                    description: None,
                    source: AuditSource::Cleanup,
                    request_id: None,
                }).await;
                if let Err(e) = audit_res {
                    error!("{} id={}: audit insert failed: {}", table, id, e.message);
                    let _ = txn.rollback().await;
                    failed += 1;
                    continue;
                }
                let del_res = sys_domain::Entity::delete_many()
                    .filter(sys_domain::Column::Id.eq(&id))
                    .exec(&txn)
                    .await;
                match del_res {
                    Err(e) => { error!("{} id={}: DELETE failed: {}", table, id, e); let _ = txn.rollback().await; failed += 1; }
                    Ok(result) => {
                        if result.rows_affected == 0 {
                            warn!("{} id={}: DELETE affected 0 rows (already gone?), skipping", table, id);
                            let _ = txn.rollback().await;
                            failed += 1;
                        } else {
                            match txn.commit().await {
                                Err(e) => { error!("{} id={}: commit failed: {}", table, id, e); failed += 1; }
                                Ok(_) => { info!("{} id={}: hard-deleted", table, id); ok += 1; }
                            }
                        }
                    }
                }
            }
        }
    }
    if execute && ok > 0 {
        metrics::counter!(
            "cleanup_job_rows_deleted_total",
            "table_name" => table
        )
        .increment(ok as u64);
    }
    SweepResult { ok, failed }
}

async fn sweep_sys_organization(
    db: &DatabaseConnection,
    actor: &Actor,
    cutoff: &NaiveDateTime,
    execute: bool,
) -> SweepResult {
    let table = "sys_organization";
    let rows = sys_organization::Entity::find()
        .filter(sys_organization::Column::DeletedAt.is_not_null())
        .filter(sys_organization::Column::DeletedAt.lt(*cutoff))
        .all(db)
        .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => { error!("{}: SELECT failed: {}", table, e); return SweepResult { ok: 0, failed: 1 }; }
    };

    if !execute {
        info!("{}", dry_run_line(table, rows.len(), cutoff));
        // In dry-run mode `ok` carries "rows that would be deleted", not rows actually deleted.
        return SweepResult { ok: rows.len(), failed: 0 };
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    for model in rows {
        let id = model.id.clone();
        let before = audit_snapshot(&model);
        match db.begin().await {
            Err(e) => { error!("{} id={}: begin txn failed: {}", table, id, e); failed += 1; }
            Ok(txn) => {
                let audit_res = audit_log::write_in_txn(&txn, AuditEvent {
                    actor,
                    operation: AuditOperation::HardDelete,
                    entity_type: "sys_organization",
                    entity_id: id.clone(),
                    payload_before: Some(before),
                    payload_after: None,
                    description: None,
                    source: AuditSource::Cleanup,
                    request_id: None,
                }).await;
                if let Err(e) = audit_res {
                    error!("{} id={}: audit insert failed: {}", table, id, e.message);
                    let _ = txn.rollback().await;
                    failed += 1;
                    continue;
                }
                let del_res = sys_organization::Entity::delete_many()
                    .filter(sys_organization::Column::Id.eq(&id))
                    .exec(&txn)
                    .await;
                match del_res {
                    Err(e) => { error!("{} id={}: DELETE failed: {}", table, id, e); let _ = txn.rollback().await; failed += 1; }
                    Ok(result) => {
                        if result.rows_affected == 0 {
                            warn!("{} id={}: DELETE affected 0 rows (already gone?), skipping", table, id);
                            let _ = txn.rollback().await;
                            failed += 1;
                        } else {
                            match txn.commit().await {
                                Err(e) => { error!("{} id={}: commit failed: {}", table, id, e); failed += 1; }
                                Ok(_) => { info!("{} id={}: hard-deleted", table, id); ok += 1; }
                            }
                        }
                    }
                }
            }
        }
    }
    if execute && ok > 0 {
        metrics::counter!(
            "cleanup_job_rows_deleted_total",
            "table_name" => table
        )
        .increment(ok as u64);
    }
    SweepResult { ok, failed }
}

async fn sweep_sys_endpoint(
    db: &DatabaseConnection,
    actor: &Actor,
    cutoff: &NaiveDateTime,
    execute: bool,
) -> SweepResult {
    let table = "sys_endpoint";
    let rows = sys_endpoint::Entity::find()
        .filter(sys_endpoint::Column::DeletedAt.is_not_null())
        .filter(sys_endpoint::Column::DeletedAt.lt(*cutoff))
        .all(db)
        .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => { error!("{}: SELECT failed: {}", table, e); return SweepResult { ok: 0, failed: 1 }; }
    };

    if !execute {
        info!("{}", dry_run_line(table, rows.len(), cutoff));
        // In dry-run mode `ok` carries "rows that would be deleted", not rows actually deleted.
        return SweepResult { ok: rows.len(), failed: 0 };
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    for model in rows {
        let id = model.id.clone();
        let before = audit_snapshot(&model);
        match db.begin().await {
            Err(e) => { error!("{} id={}: begin txn failed: {}", table, id, e); failed += 1; }
            Ok(txn) => {
                let audit_res = audit_log::write_in_txn(&txn, AuditEvent {
                    actor,
                    operation: AuditOperation::HardDelete,
                    entity_type: "sys_endpoint",
                    entity_id: id.clone(),
                    payload_before: Some(before),
                    payload_after: None,
                    description: None,
                    source: AuditSource::Cleanup,
                    request_id: None,
                }).await;
                if let Err(e) = audit_res {
                    error!("{} id={}: audit insert failed: {}", table, id, e.message);
                    let _ = txn.rollback().await;
                    failed += 1;
                    continue;
                }
                let del_res = sys_endpoint::Entity::delete_many()
                    .filter(sys_endpoint::Column::Id.eq(&id))
                    .exec(&txn)
                    .await;
                match del_res {
                    Err(e) => { error!("{} id={}: DELETE failed: {}", table, id, e); let _ = txn.rollback().await; failed += 1; }
                    Ok(result) => {
                        if result.rows_affected == 0 {
                            warn!("{} id={}: DELETE affected 0 rows (already gone?), skipping", table, id);
                            let _ = txn.rollback().await;
                            failed += 1;
                        } else {
                            match txn.commit().await {
                                Err(e) => { error!("{} id={}: commit failed: {}", table, id, e); failed += 1; }
                                Ok(_) => { info!("{} id={}: hard-deleted", table, id); ok += 1; }
                            }
                        }
                    }
                }
            }
        }
    }
    if execute && ok > 0 {
        metrics::counter!(
            "cleanup_job_rows_deleted_total",
            "table_name" => table
        )
        .increment(ok as u64);
    }
    SweepResult { ok, failed }
}

async fn sweep_sys_access_key(
    db: &DatabaseConnection,
    actor: &Actor,
    cutoff: &NaiveDateTime,
    execute: bool,
) -> SweepResult {
    let table = "sys_access_key";
    let rows = sys_access_key::Entity::find()
        .filter(sys_access_key::Column::DeletedAt.is_not_null())
        .filter(sys_access_key::Column::DeletedAt.lt(*cutoff))
        .all(db)
        .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => { error!("{}: SELECT failed: {}", table, e); return SweepResult { ok: 0, failed: 1 }; }
    };

    if !execute {
        info!("{}", dry_run_line(table, rows.len(), cutoff));
        // In dry-run mode `ok` carries "rows that would be deleted", not rows actually deleted.
        return SweepResult { ok: rows.len(), failed: 0 };
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    for model in rows {
        let id = model.id.clone();
        let before = audit_snapshot(&model);
        match db.begin().await {
            Err(e) => { error!("{} id={}: begin txn failed: {}", table, id, e); failed += 1; }
            Ok(txn) => {
                let audit_res = audit_log::write_in_txn(&txn, AuditEvent {
                    actor,
                    operation: AuditOperation::HardDelete,
                    entity_type: "sys_access_key",
                    entity_id: id.clone(),
                    payload_before: Some(before),
                    payload_after: None,
                    description: None,
                    source: AuditSource::Cleanup,
                    request_id: None,
                }).await;
                if let Err(e) = audit_res {
                    error!("{} id={}: audit insert failed: {}", table, id, e.message);
                    let _ = txn.rollback().await;
                    failed += 1;
                    continue;
                }
                let del_res = sys_access_key::Entity::delete_many()
                    .filter(sys_access_key::Column::Id.eq(&id))
                    .exec(&txn)
                    .await;
                match del_res {
                    Err(e) => { error!("{} id={}: DELETE failed: {}", table, id, e); let _ = txn.rollback().await; failed += 1; }
                    Ok(result) => {
                        if result.rows_affected == 0 {
                            warn!("{} id={}: DELETE affected 0 rows (already gone?), skipping", table, id);
                            let _ = txn.rollback().await;
                            failed += 1;
                        } else {
                            match txn.commit().await {
                                Err(e) => { error!("{} id={}: commit failed: {}", table, id, e); failed += 1; }
                                Ok(_) => { info!("{} id={}: hard-deleted", table, id); ok += 1; }
                            }
                        }
                    }
                }
            }
        }
    }
    if execute && ok > 0 {
        metrics::counter!(
            "cleanup_job_rows_deleted_total",
            "table_name" => table
        )
        .increment(ok as u64);
    }
    SweepResult { ok, failed }
}

// =============================================================================
// Pushgateway recorder install (050 044-R1)
// =============================================================================

/// 050 044-R1: 安裝 prometheus pushgateway recorder (per spec FR-005)。
/// cleanup binary 是 cron-driven 短命 process、不適合 HTTP listener (拉模式);
/// 改用 pushgateway: cleanup 跑完 push counter 到 pushgateway service、
/// prometheus scrape pushgateway 拉到 series。
///
/// endpoint 預設 `http://pushgateway:9091` (docker network alias)、可由
/// PUSHGATEWAY_URL env var 覆寫; job=cleanup、instance=$HOSTNAME (default "cleanup-binary")。
/// interval 10s background push、main() 結尾 sleep 為 flush fallback (per research R-2.3)。
fn install_pushgateway_recorder() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("PUSHGATEWAY_URL")
        .unwrap_or_else(|_| "http://pushgateway:9091".to_string());
    let job = "cleanup";
    let instance = std::env::var("HOSTNAME").unwrap_or_else(|_| "cleanup-binary".to_string());
    let url = format!(
        "{}/metrics/job/{}/instance/{}",
        endpoint.trim_end_matches('/'),
        job,
        instance
    );

    PrometheusBuilder::new()
        .with_push_gateway(
            url,
            std::time::Duration::from_secs(10),
            None, // username (no auth)
            None, // password (no auth)
        )?
        .install()?;
    Ok(())
}

// =============================================================================
// Entry point
// =============================================================================

#[tokio::main]
async fn main() {
    // metrics-exporter-prometheus push-gateway feature → reqwest → rustls 0.23+
    // requires explicit CryptoProvider install before any TLS operation; otherwise
    // background push thread panics even on plain-http URLs.
    if rustls::crypto::ring::default_provider().install_default().is_err() {
        eprintln!("rustls CryptoProvider already installed (idempotent)");
    }

    tracing_subscriber::fmt().init();

    // 050 044-R1: install pushgateway recorder 早於 metrics::counter! 呼叫 (per spec FR-005)。
    // 失敗只 warn、cleanup 主邏輯不影響 (silent no-op 比 panic 安全)。
    if let Err(e) = install_pushgateway_recorder() {
        warn!("pushgateway recorder install failed (metrics will not be pushed): {}", e);
    }

    // --- 1. Retention config ---
    let raw = std::env::var("CLEANUP_RETENTION_DAYS").ok();
    let retention_days = match parse_retention_days(raw.as_deref()) {
        Ok(d) => d,
        Err(msg) => {
            error!("Invalid CLEANUP_RETENTION_DAYS: {}", msg);
            std::process::exit(1);
        }
    };

    // --- 2. Cutoff ---
    let now = Utc::now();
    let cutoff = compute_cutoff(retention_days, now);

    // --- 3. CLI mode ---
    let execute = std::env::args().any(|a| a == "--execute");

    if execute {
        info!("cleanup --execute: retention={}d, cutoff={}", retention_days, cutoff);
    } else {
        info!("cleanup dry-run: retention={}d, cutoff={}", retention_days, cutoff);
    }

    // --- 4. DB connection ---
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(u) if u.is_empty() => {
            error!("DATABASE_URL is empty");
            std::process::exit(1);
        }
        Ok(u) => u,
        Err(_) => {
            error!("DATABASE_URL is not set");
            std::process::exit(1);
        }
    };

    let db = match sea_orm::Database::connect(&database_url).await {
        Ok(d) => d,
        Err(e) => {
            error!("DB connection failed: {}", e);
            std::process::exit(1);
        }
    };

    let actor = Actor::system("cleanup_job");

    // --- 5. Per-table sweep ---
    let results = [
        sweep_sys_user(&db, &actor, &cutoff, execute).await,
        sweep_sys_role(&db, &actor, &cutoff, execute).await,
        sweep_sys_menu(&db, &actor, &cutoff, execute).await,
        sweep_sys_domain(&db, &actor, &cutoff, execute).await,
        sweep_sys_organization(&db, &actor, &cutoff, execute).await,
        sweep_sys_endpoint(&db, &actor, &cutoff, execute).await,
        sweep_sys_access_key(&db, &actor, &cutoff, execute).await,
    ];

    // --- 6. Summary ---
    let total_ok: usize = results.iter().map(|r| r.ok).sum();
    let total_failed: usize = results.iter().map(|r| r.failed).sum();

    if execute {
        info!(
            "cleanup done: {} deleted, {} failed",
            total_ok, total_failed
        );
    } else {
        info!("dry-run total: {} row(s) would be deleted", total_ok);
    }

    // 050 044-R1: on-exit flush fallback — metrics-exporter-prometheus 0.15 push gateway 為
    // 10s background interval、無 explicit .flush() API; cleanup binary 為短命 cron process、
    // 結尾 sleep 一個 interval 保最後一批 counter push 完成再 exit (per research R-2.3)。
    // sleep 12s = interval (10s) + buffer (2s)、err side of caution。
    tokio::time::sleep(std::time::Duration::from_secs(12)).await;

    // --- 7. Exit code ---
    if total_failed > 0 {
        std::process::exit(1);
    }
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // --- parse_retention_days ---

    #[test]
    fn test_retention_unset_defaults_to_90() {
        assert_eq!(parse_retention_days(None), Ok(90));
    }

    #[test]
    fn test_retention_empty_string_is_error() {
        assert!(parse_retention_days(Some("")).is_err());
    }

    #[test]
    fn test_retention_zero_is_error() {
        assert!(parse_retention_days(Some("0")).is_err());
    }

    #[test]
    fn test_retention_negative_is_error() {
        assert!(parse_retention_days(Some("-5")).is_err());
    }

    #[test]
    fn test_retention_non_numeric_is_error() {
        assert!(parse_retention_days(Some("abc")).is_err());
    }

    #[test]
    fn test_retention_90_ok() {
        assert_eq!(parse_retention_days(Some("90")), Ok(90));
    }

    #[test]
    fn test_retention_180_ok() {
        assert_eq!(parse_retention_days(Some("180")), Ok(180));
    }

    // --- compute_cutoff ---

    #[test]
    fn test_cutoff_subtracts_correct_days() {
        // 2026-01-01 00:00:00 UTC, retention = 90 days
        // expected cutoff = 2025-10-03 00:00:00 UTC
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let cutoff = compute_cutoff(90, now);
        let expected = Utc
            .with_ymd_and_hms(2025, 10, 3, 0, 0, 0)
            .unwrap()
            .naive_utc();
        assert_eq!(cutoff, expected);
    }

    #[test]
    fn test_cutoff_zero_days_would_equal_now() {
        // retention=0 is invalid at the parse level, but compute_cutoff itself is pure:
        // 0 days subtracted → cutoff == now
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let cutoff = compute_cutoff(0, now);
        assert_eq!(cutoff, now.naive_utc());
    }

    // --- dry_run_line ---

    #[test]
    fn test_dry_run_line_format() {
        let cutoff = Utc
            .with_ymd_and_hms(2025, 10, 3, 0, 0, 0)
            .unwrap()
            .naive_utc();
        let line = dry_run_line("sys_user", 42, &cutoff);
        assert_eq!(
            line,
            "sys_user: 42 row(s) will be deleted (deleted_at < 2025-10-03 00:00:00)"
        );
    }

    #[test]
    fn test_dry_run_line_zero_count() {
        let cutoff = Utc
            .with_ymd_and_hms(2025, 10, 3, 0, 0, 0)
            .unwrap()
            .naive_utc();
        let line = dry_run_line("sys_menu", 0, &cutoff);
        assert!(line.contains("sys_menu: 0 row(s)"));
    }
}
