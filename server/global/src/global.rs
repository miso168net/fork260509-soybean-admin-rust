use std::{
    any::{Any, TypeId},
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
};

use aws_sdk_s3::Client as S3Client;
use chrono::NaiveDateTime;
use http::Method;
use jsonwebtoken::{DecodingKey, EncodingKey, Validation};
use mongodb::Client as MongoClient;
use once_cell::sync::Lazy;
use redis::{cluster::ClusterClient, Client};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tokio::sync::{mpsc, Mutex, OnceCell, RwLock};
use tracing::Instrument;

use crate::project_info;

//*****************************************************************************
// 全局配置
//*****************************************************************************

pub static GLOBAL_CONFIG: Lazy<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub async fn init_config<T: 'static + Any + Send + Sync>(config: T) {
    let mut context = GLOBAL_CONFIG.write().await;
    context.insert(TypeId::of::<T>(), Arc::new(config));
}

pub async fn get_config<T: 'static + Any + Send + Sync>() -> Option<Arc<T>> {
    let context = GLOBAL_CONFIG.read().await;
    context
        .get(&TypeId::of::<T>())
        .and_then(|config| config.clone().downcast::<T>().ok())
}

//*****************************************************************************
// 数据库连接
//*****************************************************************************

pub static GLOBAL_PRIMARY_DB: Lazy<RwLock<Option<Arc<DatabaseConnection>>>> =
    Lazy::new(|| RwLock::new(None));
pub static GLOBAL_DB_POOL: Lazy<RwLock<HashMap<String, Arc<DatabaseConnection>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

// Redis连接
#[derive(Clone)]
pub enum RedisConnection {
    Single(Arc<Client>),
    Cluster(Arc<ClusterClient>),
}

pub static GLOBAL_PRIMARY_REDIS: Lazy<RwLock<Option<RedisConnection>>> =
    Lazy::new(|| RwLock::new(None));

pub static GLOBAL_REDIS_POOL: Lazy<RwLock<HashMap<String, RedisConnection>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

// MongoDB 连接
pub static GLOBAL_PRIMARY_MONGO: Lazy<RwLock<Option<Arc<MongoClient>>>> =
    Lazy::new(|| RwLock::new(None));

pub static GLOBAL_MONGO_POOL: Lazy<RwLock<HashMap<String, Arc<MongoClient>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

//*****************************************************************************
// AWS S3 Clients
//*****************************************************************************

// 主要的 S3 客户端
pub static GLOBAL_PRIMARY_S3: Lazy<RwLock<Option<Arc<S3Client>>>> = Lazy::new(|| RwLock::new(None));

// 命名的 S3 客户端池
pub static GLOBAL_S3_POOL: Lazy<RwLock<HashMap<String, Arc<S3Client>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

//*****************************************************************************
// JWT 密钥和验证
//*****************************************************************************

pub struct Keys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl Keys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

pub static KEYS: OnceCell<Arc<Mutex<Keys>>> = OnceCell::const_new();
pub static REFRESH_KEYS: OnceCell<Arc<Mutex<Keys>>> = OnceCell::const_new();
pub static VALIDATION: OnceCell<Arc<Mutex<Validation>>> = OnceCell::const_new();

//*****************************************************************************
// 事件通道
//*****************************************************************************

/// 事件通道条目
struct DynChannelEntry {
    name: String,
    tx: mpsc::UnboundedSender<Box<dyn Any + Send>>,
}

/// 事件通道管理器
struct EventChannels {
    string_tx: mpsc::UnboundedSender<String>,
    dyn_channels: Vec<DynChannelEntry>,
}

static EVENT_CHANNELS: Lazy<Arc<Mutex<EventChannels>>> = Lazy::new(|| {
    let (string_tx, _) = mpsc::unbounded_channel();
    Arc::new(Mutex::new(EventChannels {
        string_tx,
        dyn_channels: Vec::new(),
    }))
});

type DynFuture = dyn Future<Output = ()> + Send + 'static;
type StringListener = Box<dyn FnOnce(mpsc::UnboundedReceiver<String>) -> Pin<Box<DynFuture>>>;
type DynListener = (
    String,
    Box<dyn Fn(mpsc::UnboundedReceiver<Box<dyn Any + Send>>) -> Pin<Box<DynFuture>>>,
);

/// 获取字符串事件发送器
#[inline]
pub async fn get_string_sender() -> mpsc::UnboundedSender<String> {
    EVENT_CHANNELS.lock().await.string_tx.clone()
}

/// 获取动态类型事件发送器
#[inline]
pub async fn get_dyn_sender(name: &str) -> Option<mpsc::UnboundedSender<Box<dyn Any + Send>>> {
    let channels = EVENT_CHANNELS.lock().await;
    channels
        .dyn_channels
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.tx.clone())
}

/// 注册事件监听器
pub async fn register_event_listeners(
    string_listener: StringListener,
    dyn_listeners: &[DynListener],
) {
    let mut channels = EVENT_CHANNELS.lock().await;

    // 设置字符串事件通道
    let (string_tx, string_rx) = mpsc::unbounded_channel();
    channels.string_tx = string_tx;

    // 启动字符串事件监听器
    tokio::spawn(string_listener(string_rx).instrument(tracing::Span::current()));
    project_info!("String event listener spawned");

    // 清空旧的发送器
    channels.dyn_channels.clear();

    // 为每个监听器创建独立通道
    for (name, listener) in dyn_listeners {
        let (tx, rx) = mpsc::unbounded_channel();
        channels.dyn_channels.push(DynChannelEntry {
            name: name.clone(),
            tx,
        });
        tokio::spawn(listener(rx).instrument(tracing::Span::current()));
        project_info!("Dynamic event listener '{}' spawned", name);
    }
}

//*****************************************************************************
// 路由信息收集
//*****************************************************************************

#[derive(Clone, Debug)]
pub struct RouteInfo {
    pub path: String,
    pub method: Method,
    pub service_name: String,
    pub summary: String,
}

impl RouteInfo {
    pub fn new(path: &str, method: Method, service_name: &str, summary: &str) -> Self {
        RouteInfo {
            path: path.to_string(),
            method,
            service_name: service_name.to_string(),
            summary: summary.to_string(),
        }
    }
}

pub static ROUTE_COLLECTOR: Lazy<Mutex<Vec<RouteInfo>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub async fn add_route(route: RouteInfo) {
    ROUTE_COLLECTOR.lock().await.push(route);
}

pub async fn get_collected_routes() -> Vec<RouteInfo> {
    ROUTE_COLLECTOR.lock().await.clone()
}

pub async fn clear_routes() {
    ROUTE_COLLECTOR.lock().await.clear();
}

//*****************************************************************************
// 操作日志
//*****************************************************************************

#[derive(Debug, Clone)]
pub struct OperationLogContext {
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub domain: Option<String>,
    pub module_name: String,
    pub description: String,
    pub request_id: String,
    pub method: String,
    pub url: String,
    pub ip: String,
    pub user_agent: Option<String>,
    pub params: Option<Value>,
    pub body: Option<Value>,
    pub response: Option<Value>,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
    pub duration: i32,
    pub created_at: NaiveDateTime,
}

static OPERATION_LOG_CONTEXT: Lazy<Arc<RwLock<Option<OperationLogContext>>>> =
    Lazy::new(|| Arc::new(RwLock::new(None)));

impl OperationLogContext {
    pub async fn set(context: OperationLogContext) {
        let mut writer = OPERATION_LOG_CONTEXT.write().await;
        *writer = Some(context);
    }

    pub async fn get() -> Option<OperationLogContext> {
        OPERATION_LOG_CONTEXT.read().await.clone()
    }

    pub async fn clear() {
        let mut writer = OPERATION_LOG_CONTEXT.write().await;
        *writer = None;
    }
}

/// 异步发送字符串事件
#[inline]
pub fn send_string_event(msg: String) {
    tokio::spawn(
        async move {
            let sender = get_string_sender().await;
            let _ = sender.send(msg);
        }
        .instrument(tracing::Span::current()),
    );
}

/// 异步发送动态类型事件
#[inline]
pub fn send_dyn_event(event_name: &'static str, event: Box<dyn Any + Send>) {
    tokio::spawn(
        async move {
            if let Some(sender) = get_dyn_sender(event_name).await {
                let _ = sender.send(event);
            }
        }
        .instrument(tracing::Span::current()),
    );
}

//*****************************************************************************
// 042 request_id tokio task-local（INTERNAL audit row 串聯 HTTP audit row 用）
//*****************************************************************************
//
// OperationLogLayer 從 RequestId extension 拿到 request_id 後、用
// `REQUEST_ID_TASK_LOCAL.scope(...)` 包住 inner.call(req).await；handler / service /
// `audit_log::write_in_txn` 全在同一 tokio task 內、可由 `.try_with(...)` 取得當前
// request 的 request_id。
//
// 用途：service-level audit 建構 AuditEvent 時 `request_id: None`（030-040 callsite 不改）、
// 由 write_in_txn 在 serialize 前自動 fallback 至 task-local 值。HTTP 與 INTERNAL row 因此
// 共用同一 request_id、forensic 可串聯（per spec data-model §E3）。
tokio::task_local! {
    pub static REQUEST_ID_TASK_LOCAL: String;
}

//*****************************************************************************
// 042 HTTP audit outbox writer registry
//*****************************************************************************
//
// `server-core::web::operation_log` middleware 需要呼叫 `server-model::admin::audit_log::
// write_outbox_for_http`，但 core → model 反向 dep 不允許。改採 callback registry
// pattern：模型側透過 [`register_http_audit_writer`] 註冊寫入函式；middleware 用
// [`spawn_http_audit_write`] 觸發、fire-and-forget。
//
// 註冊發生於 `server-initialize::initialize_audit_outbox_drainer`（drainer 起動同步註冊
// 一次），實際呼叫 in HTTP middleware post-execution hook。失敗只 warn、不影響 response
// （per spec FR-008）。

/// HTTP audit writer 函式簽名：取 `OperationLogContext`、async 寫 outbox、回 unit/Result。
/// Box::pin async block 為 trait-object-friendly 形式。
pub type HttpAuditWriter = Box<
    dyn Fn(OperationLogContext) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
        + Send
        + Sync,
>;

static HTTP_AUDIT_WRITER: Lazy<RwLock<Option<HttpAuditWriter>>> =
    Lazy::new(|| RwLock::new(None));

/// 042: 註冊 HTTP audit outbox writer（initialize 階段呼叫一次）。
pub async fn register_http_audit_writer(writer: HttpAuditWriter) {
    let mut guard = HTTP_AUDIT_WRITER.write().await;
    *guard = Some(writer);
}

/// 042: middleware 用 — fire-and-forget spawn writer 寫 outbox；無 writer 註冊則 warn。
/// 失敗只 log warn、不返 error、不影響 response（per spec FR-008）。
///
/// 044 W-F12 FR-004：用 `.instrument(tracing::Span::current())` 把當前 `http_req`
/// span（含 `request_id` attr）傳遞給 spawn 出去的 task,讓 warn! log 能繼承 request_id。
#[inline]
pub fn spawn_http_audit_write(ctx: OperationLogContext) {
    tokio::spawn(
        async move {
            let guard = HTTP_AUDIT_WRITER.read().await;
            match guard.as_ref() {
                Some(writer) => {
                    if let Err(e) = writer(ctx).await {
                        tracing::warn!(
                            target: "operation_log_middleware",
                            error = %e,
                            "HTTP audit outbox write failed (response unaffected)"
                        );
                    }
                }
                None => {
                    tracing::warn!(
                        target: "operation_log_middleware",
                        "HTTP_AUDIT_WRITER 未註冊、middleware audit 寫入跳過"
                    );
                }
            }
        }
        .instrument(tracing::Span::current()),
    );
}
