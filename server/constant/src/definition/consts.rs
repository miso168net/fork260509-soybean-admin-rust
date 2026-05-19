use strum_macros::{AsRefStr, Display, EnumString};

/// Token 状态枚举
#[derive(Debug, Clone, PartialEq, Eq, AsRefStr, Display, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum TokenStatus {
    /// 活跃状态，可以正常使用
    #[strum(serialize = "unused")]
    Active,
    /// 已被刷新，表示该 token 已被新 token 替换
    #[strum(serialize = "used")]
    Refreshed,
    /// 已被撤销（手动注销或安全原因）
    Revoked,
}

impl TokenStatus {
    pub fn is_valid(&self) -> bool {
        matches!(self, TokenStatus::Active)
    }

    pub fn can_refresh(&self) -> bool {
        matches!(self, TokenStatus::Active)
    }
}

/// 系统事件类型枚举
#[derive(Debug, Clone, PartialEq, Eq, AsRefStr, Display, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum SystemEvent {
    /// 用户认证登录事件
    AuthLoggedInEvent,
    /// 系统操作日志事件
    AuditOperationLoggedEvent,
    /// API密钥验证事件
    AuthApiKeyValidatedEvent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_token_status_serialize_aligns_with_nestjs() {
        // forward: Display via strum derive(.to_string() 走 std blanket impl)
        assert_eq!(TokenStatus::Active.to_string(), "unused");
        assert_eq!(TokenStatus::Refreshed.to_string(), "used");
        assert_eq!(TokenStatus::Revoked.to_string(), "revoked");

        // reverse: EnumString from_str(per R-Q5 strum 對稱性)
        assert_eq!(TokenStatus::from_str("unused").unwrap(), TokenStatus::Active);
        assert_eq!(TokenStatus::from_str("used").unwrap(), TokenStatus::Refreshed);
        assert_eq!(TokenStatus::from_str("revoked").unwrap(), TokenStatus::Revoked);
    }
}
