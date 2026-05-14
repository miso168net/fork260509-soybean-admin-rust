//! Business code constants for the `Res<T>` response envelope.
//!
//! All 24 codes from specs/001-response-shape-alignment/spec.md §Key Entities,
//! grouped: success (1) / logout (2) / modal logout (2) / expired (3) /
//! validation (3) / permission (4) / business (4) / server (5) — sums to 24.
//! (Spec FR-005 summary "23 條" 為 arithmetic typo — 以本檔 + data-model.md §E2 列表為準。)
//!
//! Naming: `CODE_<group>_<semantic>` (R2 — SCREAMING_SNAKE_CASE, flat namespace).
//! Reference: specs/001-response-shape-alignment/data-model.md §E2.

// Success (1 條)
pub const CODE_SUCCESS: u16 = 0;

// Logout (2 條) — base 既有
pub const CODE_LOGOUT_SESSION_INVALIDATED: u16 = 8888;
pub const CODE_LOGOUT_CREDENTIAL_ROTATED: u16 = 8889;

// Modal Logout (2 條) — base 既有
pub const CODE_MODAL_LOGOUT_CONCURRENT_LOGIN: u16 = 7777;
pub const CODE_MODAL_LOGOUT_ACCOUNT_SUSPENDED: u16 = 7778;

// Expired Token (3 條) — base 既有
pub const CODE_EXPIRED_ACCESS_TOKEN: u16 = 9999;
pub const CODE_EXPIRED_REFRESH_TOKEN: u16 = 9998;
pub const CODE_EXPIRED_TOKEN_SIGNATURE: u16 = 3333;

// Validation (3 條) — rust 新
pub const CODE_VALIDATION_REQUIRED_FIELD: u16 = 4001;
pub const CODE_VALIDATION_FORMAT_INVALID: u16 = 4002;
pub const CODE_VALIDATION_CONSTRAINT_VIOLATED: u16 = 4003;

// Permission (4 條) — rust 新
pub const CODE_PERMISSION_CASBIN_DENY: u16 = 5001;
pub const CODE_PERMISSION_ROLE_INSUFFICIENT: u16 = 5002;
pub const CODE_PERMISSION_API_KEY_MISSING: u16 = 5003;
pub const CODE_PERMISSION_API_KEY_SIGNATURE_INVALID: u16 = 5004;

// Business (4 條) — rust 新
pub const CODE_BUSINESS_ENTITY_NOT_FOUND: u16 = 6001;
pub const CODE_BUSINESS_DUPLICATE_VIOLATION: u16 = 6002;
pub const CODE_BUSINESS_STATE_CONFLICT: u16 = 6003;
pub const CODE_BUSINESS_DEPENDENCY_MISSING: u16 = 6004;

// Server (5 條) — rust 新
pub const CODE_SERVER_DB_ERROR: u16 = 9001;
pub const CODE_SERVER_CACHE_ERROR: u16 = 9002;
pub const CODE_SERVER_EXTERNAL_SERVICE_ERROR: u16 = 9003;
pub const CODE_SERVER_CONFIGURATION_ERROR: u16 = 9004;
pub const CODE_SERVER_INTERNAL_ERROR: u16 = 9005;
