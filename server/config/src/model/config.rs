use serde::Deserialize;

use super::{
    AuditOutboxConfig, DatabaseConfig, DatabasesInstancesConfig, JwtConfig, MongoConfig,
    MongoInstancesConfig, RedisConfig, RedisInstancesConfig, S3Config, S3InstancesConfig,
    ServerConfig,
};

/// 应用程序配置结构
///
/// 这是应用程序的主配置结构，包含了所有子系统的配置信息
///
/// ## 配置加载优先级
/// 1. **环境变量**（最高优先级）
/// 2. **配置文件**（中等优先级）
/// 3. **默认值**（最低优先级）
///
/// ## 环境变量命名规范
/// - 使用 `APP_` 前缀
/// - 嵌套配置用下划线分隔，如：`APP_DATABASE_URL`
/// - 数组配置用索引，如：`APP_REDIS_INSTANCES_0_NAME`
///
/// ## 配置的加载和初始化过程
/// 1. 首先从配置文件（如 application.yaml）中加载整体配置
/// 2. 然后从环境变量中读取配置并覆盖文件配置
/// 3. 最后通过 `init_from_file_with_env` 函数将配置注入到全局状态中
///    ```rust,no_run
///    use server_global::global;
///    use server_config::{Config, DatabaseConfig, ServerConfig, JwtConfig, RedisConfig};
///
///    async fn init_config_example(config: Config) {
///        // 注入主配置
///        global::init_config::<Config>(config.clone()).await;
///
///        // 注入数据库配置
///        global::init_config::<DatabaseConfig>(config.database).await;
///
///        // 注入服务器配置
///        global::init_config::<ServerConfig>(config.server).await;
///
///        // 注入 JWT 配置
///        global::init_config::<JwtConfig>(config.jwt).await;
///
///        // 注入 Redis 配置（如果存在）
///        if let Some(redis_config) = config.redis {
///            global::init_config::<RedisConfig>(redis_config).await;
///        }
///    }
///    ```
///
/// # 配置项说明
///
/// - `database`: 主数据库配置，用于配置默认的数据库连接
/// - `database_instances`: 可选的数据库连接池配置，用于配置多个命名的数据库连接
/// - `server`: HTTP 服务器配置，包含监听地址和端口等
/// - `jwt`: JWT 认证配置，包含密钥和过期时间等
/// - `redis`: 主 Redis 配置，用于配置默认的 Redis 连接
/// - `redis_instances`: 可选的 Redis 连接池配置，用于配置多个命名的 Redis 连接
/// - `mongo`: 主 MongoDB 配置，用于配置默认的 MongoDB 连接
/// - `mongo_instances`: 可选的 MongoDB 连接池配置，用于配置多个命名的 MongoDB 连接
///
/// # 示例配置（YAML）
/// ```yaml
/// database:
///   url: "postgres://user:pass@localhost:5432/dbname"
///   max_connections: 10
///
/// database_instances:
///   - name: "other_db"
///     url: "postgres://user:pass@localhost:5432/other_db"
///
/// server:
///   host: "127.0.0.1"
///   port: 8080
///
/// jwt:
///   secret: "your-secret-key"
///   expire: 3600
///
/// redis:
///   mode: "single"
///   url: "redis://:password@localhost:6379/0"
///
/// redis_instances:
///   - name: "cache"
///     mode: "cluster"
///     urls:
///       - "redis://:password@localhost:6379"
///       - "redis://:password@localhost:6380"
/// ```
#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// 主数据库配置
    pub database: DatabaseConfig,

    /// 可选的数据库连接池配置
    /// 用于配置多个命名的数据库连接
    pub database_instances: Option<Vec<DatabasesInstancesConfig>>,

    /// HTTP 服务器配置
    pub server: ServerConfig,

    /// JWT 认证配置
    pub jwt: JwtConfig,

    /// 主 Redis 配置
    pub redis: Option<RedisConfig>,

    /// 可选的 Redis 连接池配置
    /// 用于配置多个命名的 Redis 连接
    pub redis_instances: Option<Vec<RedisInstancesConfig>>,

    /// 主 MongoDB 配置
    pub mongo: Option<MongoConfig>,

    /// 可选的 MongoDB 连接池配置
    /// 用于配置多个命名的 MongoDB 连接
    pub mongo_instances: Option<Vec<MongoInstancesConfig>>,

    /// 主 S3 配置
    pub s3: Option<S3Config>,

    /// 可选的 S3 连接池配置
    /// 用于配置多个命名的 S3 连接
    pub s3_instances: Option<Vec<S3InstancesConfig>>,

    /// 042 audit-outbox-and-http-mount: audit_outbox drainer + Redis Stream publisher 配置
    pub audit_outbox: Option<AuditOutboxConfig>,
}
