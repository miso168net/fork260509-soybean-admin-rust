//! Snowflake i64 generator for 5 業務 entity display_id（feature 039）。
//! Structure: 41bit timestamp ms from EPOCH_2020 + 5bit machine_id + 7bit sequence
//! (= 53bit total, 完全填滿 JS Number.MAX_SAFE_INTEGER = 2^53 - 1)。
//! machine_id 從 HOSTNAME env djb2 hash mod 32 取（自動、無 manual config、跟 W-F11 多 replica 預備對齊）。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const EPOCH_2020_MS: u64 = 1577836800000; // 2020-01-01 UTC
const MACHINE_BITS: u64 = 5;
const SEQ_BITS: u64 = 7;
const SEQ_MASK: u64 = (1 << SEQ_BITS) - 1;
const MAX_MACHINE_ID: u64 = (1 << MACHINE_BITS) - 1;

static MACHINE_ID: OnceLock<u64> = OnceLock::new();
static LAST_STATE: AtomicU64 = AtomicU64::new(0); // (timestamp_ms << SEQ_BITS) | seq

fn machine_id() -> u64 {
    *MACHINE_ID.get_or_init(|| {
        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "rev1-default".to_string());
        let mut hash: u64 = 5381;
        for b in hostname.as_bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(*b as u64);
        }
        hash & MAX_MACHINE_ID
    })
}

pub fn next_display_id() -> i64 {
    loop {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let now_ms = now_ms.saturating_sub(EPOCH_2020_MS);
        let prev = LAST_STATE.load(Ordering::Acquire);
        let prev_ts = prev >> SEQ_BITS;
        let prev_seq = prev & SEQ_MASK;
        let (ts, seq) = if now_ms > prev_ts {
            (now_ms, 0)
        } else if now_ms == prev_ts && prev_seq < SEQ_MASK {
            (now_ms, prev_seq + 1)
        } else {
            // clock 倒退 OR seq 用完、等到下一個 ms
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        };
        let new_state = (ts << SEQ_BITS) | seq;
        if LAST_STATE
            .compare_exchange(prev, new_state, Ordering::Release, Ordering::Acquire)
            .is_ok()
        {
            return ((ts << (MACHINE_BITS + SEQ_BITS)) | (machine_id() << SEQ_BITS) | seq) as i64;
        }
        // CAS lost → retry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_unique_and_time_ordered() {
        let mut ids = Vec::with_capacity(1000);
        for _ in 0..1000 {
            ids.push(next_display_id());
        }
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 1000, "1000 个连续生成的 display_id MUST 不重复");
        for w in ids.windows(2) {
            assert!(w[0] < w[1], "MUST time-ordered: {} < {}", w[0], w[1]);
        }
        // 範圍檢查: < 2^53 JS safe integer
        for id in &ids {
            assert!(
                *id > 0 && (*id as u64) < (1u64 << 53),
                "id {} 必須在 JS safe integer 範圍",
                id
            );
        }
    }
}
