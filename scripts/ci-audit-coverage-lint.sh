#!/usr/bin/env bash
# F2.1 CI lint：守 service 內 create_*/update_* method 必有 audit 呼叫
#
# Scope: server/service/src/admin/sys_*_service.rs 內每個 `async fn create_<x>` /
#   `async fn update_<x>` method body 範圍內必有以下其中一種命中：
#     - `audit_log::write_in_txn` 字串
#     - `AuditEvent` struct literal / import
#     - `_in_transaction(` delegation call（明示把 audit 委派給 helper）
#
# Exit 0: 全 service create/update method 都有 audit 覆蓋
# Exit 1: 任一 method 漏覆蓋、列出 file:line + method signature
#
# Whitelist（grep 範圍外、自然 skip）:
#   - sys_organization_service.rs — F2.1 read-only、無 create/update 入口
#   - sys_auth_service.rs / sys_authorization_service.rs / sys_login_log_service.rs /
#     sys_operation_log_service.rs — 非 7 admin entity CRUD（per spec FR-016）
#   - F3 既有 facade soft_delete_by_id / restore_by_id — 在 server/model/ 內、
#     不在本 grep 範圍內、自然 whitelist
#
# per F2.1 spec FR-016/017/018 + contracts/internal-api.md C9 + data-model.md §E10

set -euo pipefail

# 取 script 自身目錄、回退一層到 rust-api root（即使從別處呼叫也能對齊）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_API_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${RUST_API_ROOT}"

MISSING=""
for f in server/service/src/admin/sys_*_service.rs; do
  case "$(basename "$f")" in
    sys_organization_service.rs|sys_auth_service.rs|sys_authorization_service.rs|sys_login_log_service.rs|sys_operation_log_service.rs)
      continue
      ;;
  esac

  # awk 跨 brace depth 偵測 create_/update_ method body 範圍 + 內部 audit 覆蓋。
  #
  # State machine:
  #   - 命中 `async fn (create_|update_)<name>` → saw_signature_open=1，等待 method body
  #     開始（signature 可能跨多行）
  #   - signature 行內遇 `;` 結尾 → trait declaration（無 body）、reset state、跳過
  #   - signature 行內遇 `{` → body 開始、進入 brace-depth 追蹤模式
  #   - body 內任一行命中 audit pattern → has_audit=1
  #   - brace depth 歸 0 → body 結束、若無 audit 命中即報錯
  result=$(awk -v file="$f" '
    /^[[:space:]]*async fn (create_|update_)[a-z_]+/ {
      pending_method = $0
      gsub(/^[[:space:]]+/, "", pending_method)
      pending_line = NR
      saw_signature_open = 1
      body_started = 0
      brace_depth = 0
      has_audit = 0
      next
    }
    saw_signature_open && /;[[:space:]]*$/ {
      # trait method declaration、signature ends with ;
      saw_signature_open = 0
      next
    }
    saw_signature_open && /\{/ {
      body_started = 1
      saw_signature_open = 0
      open_count = gsub(/\{/, "{")
      close_count = gsub(/\}/, "}")
      brace_depth = open_count - close_count
      if (/(audit_log::write_in_txn|AuditEvent|_in_transaction\()/) has_audit = 1
      if (brace_depth <= 0) {
        if (!has_audit) print file ":" pending_line ": " pending_method " — missing audit_log::write_in_txn / AuditEvent / *_in_transaction delegation"
        body_started = 0
      }
      next
    }
    body_started {
      open_count = gsub(/\{/, "{")
      close_count = gsub(/\}/, "}")
      brace_depth += open_count - close_count
      if (/(audit_log::write_in_txn|AuditEvent|_in_transaction\()/) has_audit = 1
      if (brace_depth <= 0) {
        if (!has_audit) print file ":" pending_line ": " pending_method " — missing audit_log::write_in_txn / AuditEvent / *_in_transaction delegation"
        body_started = 0
      }
    }
    END {
      if (body_started && !has_audit) {
        print file ":" pending_line ": " pending_method " — missing audit_log::write_in_txn / AuditEvent / *_in_transaction delegation (EOF reached)"
      }
    }
  ' "$f")

  if [ -n "$result" ]; then
    MISSING="${MISSING}${result}"$'\n'
  fi
done

if [ -n "${MISSING}" ]; then
  echo "❌ Service create_*/update_* methods missing audit coverage:"
  printf '%s' "${MISSING}"
  exit 1
fi

echo "✅ ci-audit-coverage-lint pass"
