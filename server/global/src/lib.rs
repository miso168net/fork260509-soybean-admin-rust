pub use jsonwebtoken::Validation;

pub mod casbin_notify;
pub mod global;
pub mod snowflake;

pub use casbin_notify::{notify_casbin_changed, CASBIN_INVALIDATE_CHANNEL};

#[macro_export]
macro_rules! project_info {
    ($($arg:tt)+) => {{
        let span = tracing::span!(
            tracing::Level::INFO,
            module_path!(),
            file = file!(),
            line = line!(),
        );
        let _enter = span.enter();
        tracing::info!(
            target: "[soybean-admin-rust]",
            $($arg)+
        );
    }}
}

#[macro_export]
macro_rules! project_error {
    ($($arg:tt)+) => {{
        let span = tracing::span!(
            tracing::Level::ERROR,
            module_path!(),
            file = file!(),
            line = line!(),
        );
        let _enter = span.enter();
        tracing::error!(
            target: "[soybean-admin-rust]",
            $($arg)+
        );
    }}
}
