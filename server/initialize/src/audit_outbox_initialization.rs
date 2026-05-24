//! 042 audit-outbox-and-http-mount: spawn audit_outbox drainer 背景 task。
//!
//! per spec FR-002 + quickstart §4.3 + research R-9。
//! main.rs 在 `init_redis_pools()` 後呼叫此 fn（drainer 需 DB + Redis 都 ready）。
//!
//! 取 `AuditOutboxConfig`：若 application.yaml 未配 `audit_outbox:` section
//! 則用 `AuditOutboxConfig::default()`（batch=100, sleep=100ms, max_retry=5, maxlen=10000）。

use server_config::{AuditOutboxConfig, Config};
use server_global::global::{get_config, register_http_audit_writer};
use server_model::admin::audit_log;
use server_service::admin::run_drainer_loop;

/// 啟動 audit_outbox drainer 背景迴圈（tokio::spawn detached task）+ 註冊 HTTP audit writer callback。
///
/// 兩件事一起做：
/// 1. 註冊 `audit_log::write_outbox_for_http` 為 global HTTP audit writer
///    （middleware spawn_http_audit_write 呼叫此 callback、避免 core→model 反向 dep）。
/// 2. spawn drainer 主迴圈消化 sys_audit_outbox row。
pub async fn initialize_audit_outbox_drainer() {
    // 1. 註冊 HTTP audit writer callback
    register_http_audit_writer(Box::new(|ctx| {
        Box::pin(async move {
            audit_log::write_outbox_for_http(ctx)
                .await
                .map_err(|e| e.message)
        })
    }))
    .await;
    tracing::info!(
        target: "[soybean-admin-rust]",
        "initialize_audit_outbox_drainer: HTTP audit writer callback registered"
    );

    // 2. 取 config、spawn drainer 主迴圈
    let config = match get_config::<Config>().await {
        Some(cfg) => cfg.audit_outbox.clone().unwrap_or_default(),
        None => {
            tracing::warn!(
                target: "[soybean-admin-rust]",
                "initialize_audit_outbox_drainer: Config 未初始化、用 AuditOutboxConfig::default()"
            );
            AuditOutboxConfig::default()
        }
    };

    tracing::info!(
        target: "[soybean-admin-rust]",
        "initialize_audit_outbox_drainer: spawning drainer with config={:?}",
        config
    );

    tokio::spawn(async move {
        run_drainer_loop(config).await;
    });
}
