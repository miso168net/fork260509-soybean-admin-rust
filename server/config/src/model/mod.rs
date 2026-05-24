pub use audit_outbox_config::AuditOutboxConfig;
pub use config::Config;
pub use database_config::{DatabaseConfig, DatabasesInstancesConfig};
pub use jwt_config::JwtConfig;
pub use mongo_config::{MongoConfig, MongoInstancesConfig};
pub use redis_config::{RedisConfig, RedisInstancesConfig, RedisMode};
pub use s3_config::{S3Config, S3InstancesConfig};
pub use server_config::ServerConfig;

/// 可选配置集合的包装类
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OptionalConfigs<T> {
    pub configs: Option<Vec<T>>,
}

impl<T> From<Option<Vec<T>>> for OptionalConfigs<T> {
    fn from(configs: Option<Vec<T>>) -> Self {
        Self { configs }
    }
}

mod audit_outbox_config;
mod config;
mod database_config;
mod jwt_config;
mod mongo_config;
mod redis_config;
mod s3_config;
mod server_config;
