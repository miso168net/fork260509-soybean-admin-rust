//! F1.1 jwt-secrets unit tests
//! per spec SC-001 ~ SC-004 + spec FR-001 ~ FR-009

use server_config::secret_loader::{
    load_secret_from_file_if_set, validate_jwt_secret, PLACEHOLDER_SECRETS,
};

// ─── validate_jwt_secret tests ────────────────────────────────────────────────

#[test]
#[should_panic(expected = "empty")]
fn validate_empty_secret_panics() {
    validate_jwt_secret("");
}

/// 每個 PLACEHOLDER_SECRETS 黑名單值都應觸發 panic（含 "placeholder" in panic msg）
#[test]
fn validate_each_placeholder_secret_panics() {
    for &placeholder in PLACEHOLDER_SECRETS {
        let result = std::panic::catch_unwind(|| validate_jwt_secret(placeholder));
        assert!(
            result.is_err(),
            "expected panic for placeholder '{}' but it did not panic",
            placeholder
        );
        // 驗 panic 訊息含 "placeholder"（非空 + 非長度不足的 blacklist 值）
        if let Err(payload) = result {
            let msg = payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("");
            assert!(
                msg.contains("placeholder"),
                "panic message for '{}' should contain 'placeholder', got: {}",
                placeholder,
                msg
            );
        }
    }
}

#[test]
#[should_panic(expected = "length")]
fn validate_short_secret_panics() {
    let short = "a".repeat(31);
    validate_jwt_secret(&short);
}

/// 邊界 32 chars + 64-char hex sample 都應通過 validation（不 panic）
#[test]
fn validate_valid_secrets_ok() {
    // 邊界值：恰好 32 chars
    validate_jwt_secret(&"a".repeat(32));
    // 64-char hex（openssl rand -hex 32 產生的格式）
    validate_jwt_secret("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
}

// ─── load_secret_from_file_if_set tests ───────────────────────────────────────

/// _FILE envvar 指向有效檔案、返回 trim 後內容（"\n" 去掉）
#[test]
fn file_loader_reads_trimmed_content() {
    let path = std::env::temp_dir()
        .join(format!("f1_1_unit_trim_{}", std::process::id()));
    std::fs::write(&path, "abc\n").unwrap();
    std::env::set_var("F1_1_UNIT_TRIM_FILE", path.to_str().unwrap());

    let value = load_secret_from_file_if_set("F1_1_UNIT_TRIM");

    std::env::remove_var("F1_1_UNIT_TRIM_FILE");
    std::fs::remove_file(&path).unwrap();

    assert_eq!(value, Some("abc".to_string()));
}

/// _FILE envvar 指向不存在的檔案 → panic with "read failed"
#[test]
#[should_panic(expected = "read failed")]
fn file_loader_panics_on_missing_file() {
    std::env::set_var("F1_1_UNIT_MISSING_FILE", "/nonexistent/path/f1_1_test_xyz123abc");
    // panic 後 env var 清理交由 OS process 結束時回收（#[should_panic] test 每個在子執行緒跑）
    let _ = load_secret_from_file_if_set("F1_1_UNIT_MISSING");
}

/// _FILE envvar 未設 → 返回 None
#[test]
fn file_loader_returns_none_when_envvar_unset() {
    // 確保 _FILE var 不存在
    std::env::remove_var("F1_1_UNIT_UNSET_FILE");
    let value = load_secret_from_file_if_set("F1_1_UNIT_UNSET");
    assert_eq!(value, None);
}

/// _FILE + bare envvar 都設時，load_secret_from_file_if_set 取 _FILE 內容（precedence）
///
/// 注：實際 precedence 整合在 config_init.rs；本 test 只驗 file loader 獨立行為：
/// _FILE set 時 loader 返回 file 內容（不是 bare envvar 值）。
#[test]
fn file_envvar_precedence_over_bare() {
    let path = std::env::temp_dir()
        .join(format!("f1_1_unit_prec_{}", std::process::id()));
    let file_secret = "file-secret-value-padded-to-thirty-tw";  // 38 chars，非 placeholder
    std::fs::write(&path, file_secret).unwrap();

    std::env::set_var("F1_1_UNIT_PREC_FILE", path.to_str().unwrap());
    std::env::set_var("F1_1_UNIT_PREC", "bare-envvar-value-should-not-be-returned");

    let value = load_secret_from_file_if_set("F1_1_UNIT_PREC");

    std::env::remove_var("F1_1_UNIT_PREC_FILE");
    std::env::remove_var("F1_1_UNIT_PREC");
    std::fs::remove_file(&path).unwrap();

    assert_eq!(value, Some(file_secret.to_string()));
}
