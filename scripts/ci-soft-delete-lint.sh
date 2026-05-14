#!/usr/bin/env bash
# F3 soft-delete CI lint：禁止 service / api / router 層直接 import 7 個 SoftDeletable
# entity 的 entities path（含 prelude）—強制透過 facade::sys_<entity>。
#
# Whitelist: server/model/src/admin/facade/, server/model/src/admin/soft_delete_impls.rs
# 因為 facade 自身與 SoftDeletable impl 需要 import entities（自然 whitelist、不在
# grep 範圍內）。
#
# per F3 spec FR-017/018/019 + contracts/internal-api.md C7。

set -euo pipefail

# 取 script 自身目錄、回退一層到 rust-api root（即使從別處呼叫也能對齊）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_API_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${RUST_API_ROOT}"

# Pattern A：直 import sys_<entity> 路徑
PATTERN_A='use server_model::admin::entities::sys_(user|role|menu|domain|organization|endpoint|access_key)\b'

# Pattern B：透過 prelude alias
PATTERN_B='use server_model::admin::entities::prelude::Sys(User|Role|Menu|Domain|Organization|Endpoint|AccessKey)\b'

FOUND_A=$(grep -rE "${PATTERN_A}" server/service server/api server/router --include='*.rs' 2>/dev/null || true)
FOUND_B=$(grep -rE "${PATTERN_B}" server/service server/api server/router --include='*.rs' 2>/dev/null || true)

if [ -n "${FOUND_A}" ] || [ -n "${FOUND_B}" ]; then
  echo "❌ Direct entities import detected (must use facade::sys_<entity> instead):"
  if [ -n "${FOUND_A}" ]; then
    echo "--- Pattern A: entities::sys_<entity> ---"
    echo "${FOUND_A}"
  fi
  if [ -n "${FOUND_B}" ]; then
    echo "--- Pattern B: entities::prelude::Sys<Entity> ---"
    echo "${FOUND_B}"
  fi
  exit 1
fi

echo "✅ ci-soft-delete-lint pass"
