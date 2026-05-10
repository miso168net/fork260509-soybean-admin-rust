# new-admin-rust-api 分支來源紀錄

本 `new-admin-rust-api` 分支建立於 **2026-05-10**,作為新的 default branch,來自此 repo 既有的 `main` 分支。

| 項目 | 內容 |
|---|---|
| 此 repo | `miso168net/fork260509-soybean-admin-rust` |
| 原始專案 | `soybeanjs/soybean-admin-rust` |
| Fork 用途 | 個人學習與實驗用 fork;作為 `new-admin-root` 整合 workspace 的 `admin-api/` worktree 來源 |
| Fork 建立日 | 2026-05-09 |
| `new-admin-rust-api` 來源分支 | `main` |
| 建立時的來源 HEAD | `a560191` — ⬆️ chore: update dependencies(2025-08-11) |
| 原本的 default branch | `main` |
| 改用 new-admin-rust-api 的原因 | 為 `new-admin-root` 傘狀 monorepo 整合預備 — `admin-api/` 是這個 fork 的 worktree,整合改動全部收進專屬分支,`main` 保持乾淨以便日後追上游 rebase。 |

## 歷史說明

- 原本的 default branch `main` 完整保留,沒有刪除或修改。
- `new-admin-rust-api` 是新建的分支,從 `main` 拉出,目的是隔離整合工作與上游同步。
- 過程中沒有 squash、rebase 或改寫任何 commit 歷史。

## 如何比對 new-admin-rust-api 與 main 的差異

```bash
git log main..new-admin-rust-api --oneline      # 只在 new-admin-rust-api、不在 main 的 commit
git log new-admin-rust-api..main --oneline      # 只在 main、不在 new-admin-rust-api 的 commit
git diff main new-admin-rust-api -- .           # 兩條分支的內容差異
```

## 注意事項

本檔案只記錄 fork 的元資訊,不影響任何程式邏輯,可以安全忽略或刪除。
