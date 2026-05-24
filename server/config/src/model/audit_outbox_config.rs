//! 042 audit-outbox-and-http-mount: audit_outbox drainer 與 Redis Stream publisher 配置。
//!
//! per spec 042 FR-002 / FR-007、research.md R-8。
//! - drainer_batch_size: 每輪 SELECT FOR UPDATE SKIP LOCKED LIMIT N
//! - drainer_sleep_interval_ms: 撈空 batch 後 sleep 多久再撈下一輪
//! - drainer_max_retry: row.retry_count 達此值後停止重試
//! - redis_stream_maxlen_approx: XADD MAXLEN ~ N（approximate trim、避免 stream 無限長）

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuditOutboxConfig {
    pub drainer_batch_size: u64,
    pub drainer_sleep_interval_ms: u64,
    pub drainer_max_retry: u32,
    pub redis_stream_maxlen_approx: usize,
}

impl Default for AuditOutboxConfig {
    fn default() -> Self {
        Self {
            drainer_batch_size: 100,
            drainer_sleep_interval_ms: 100,
            drainer_max_retry: 5,
            redis_stream_maxlen_approx: 10000,
        }
    }
}
