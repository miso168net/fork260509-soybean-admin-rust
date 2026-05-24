use server_constant::definition::consts::SystemEvent;
use server_global::global;

pub async fn initialize_event_channel() {
    // 042 audit-outbox-and-http-mount: AuditOperationLoggedEvent / sys_operation_log_listener
    // 退役（HTTP audit 改走 OperationLogLayer middleware → spawn_http_audit_write →
    // audit_log::write_outbox_for_http、不再走 event channel）。
    // 其餘 3 個 listener（auth/jwt/api_key）保留不動。
    use server_service::admin::{
        api_key_validate_listener, auth_login_listener, jwt_created_listener,
    };

    global::register_event_listeners(
        Box::new(|rx| Box::pin(jwt_created_listener(rx))),
        &[
            (
                SystemEvent::AuthLoggedInEvent.to_string(),
                Box::new(|rx| Box::pin(auth_login_listener(rx))),
            ),
            (
                SystemEvent::AuthApiKeyValidatedEvent.to_string(),
                Box::new(|rx| Box::pin(api_key_validate_listener(rx))),
            ),
        ],
    )
    .await;
}
