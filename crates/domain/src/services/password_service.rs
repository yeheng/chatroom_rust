//! 密码服务
//!
//! 提供密码哈希和验证功能，使用 bcrypt 算法确保安全性。

use crate::errors::{DomainError, DomainResult};
use bcrypt::{hash, verify, DEFAULT_COST};

/// 密码服务
pub struct PasswordService;

impl PasswordService {
    /// 使用 bcrypt 对密码进行哈希
    ///
    /// # 参数
    /// - `password`: 明文密码
    ///
    /// # 返回
    /// - `Ok(String)`: 哈希后的密码
    /// - `Err(DomainError)`: 哈希失败的错误
    ///
    /// # 示例
    /// ```
    /// use domain::PasswordService;
    ///
    /// let hash = PasswordService::hash_password("my_secure_password").unwrap();
    /// assert!(hash.starts_with("$2b$"));
    /// ```
    pub fn hash_password(password: &str) -> DomainResult<String> {
        // 验证密码格式
        Self::validate_password(password)?;

        // 使用 cost = 12 确保安全性（默认值为 12）
        hash(password, DEFAULT_COST).map_err(|e| DomainError::PasswordHashError {
            message: e.to_string(),
        })
    }

    /// 使用自定义 cost 对密码进行哈希
    ///
    /// # 参数
    /// - `password`: 明文密码
    /// - `cost`: bcrypt cost 参数（推荐 10-14）
    ///
    /// # 返回
    /// - `Ok(String)`: 哈希后的密码
    /// - `Err(DomainError)`: 哈希失败的错误
    pub fn hash_password_with_cost(password: &str, cost: u32) -> DomainResult<String> {
        // 验证密码格式
        Self::validate_password(password)?;

        // 验证 cost 参数范围
        if !(10..=14).contains(&cost) {
            return Err(DomainError::validation_error(
                "cost",
                "bcrypt cost 必须在 4-31 范围内",
            ));
        }

        hash(password, cost).map_err(|e| DomainError::PasswordHashError {
            message: e.to_string(),
        })
    }

    /// 验证密码是否匹配哈希值
    ///
    /// # 参数
    /// - `password`: 明文密码
    /// - `hash`: 哈希值
    ///
    /// # 返回
    /// - `Ok(true)`: 密码匹配
    /// - `Ok(false)`: 密码不匹配
    /// - `Err(DomainError)`: 验证过程中出错
    ///
    /// # 示例
    /// ```
    /// use domain::PasswordService;
    ///
    /// let password = "my_secure_password";
    /// let hash = PasswordService::hash_password(password).unwrap();
    ///
    /// assert!(PasswordService::verify_password(password, &hash).unwrap());
    /// assert!(!PasswordService::verify_password("wrong_password", &hash).unwrap());
    /// ```
    pub fn verify_password(password: &str, hash: &str) -> DomainResult<bool> {
        verify(password, hash).map_err(|_| DomainError::PasswordVerificationError)
    }

    /// 检查密码是否需要重新哈希（cost 值更新时）
    ///
    /// # 参数
    /// - `hash`: 当前的哈希值
    /// - `target_cost`: 目标 cost 值
    ///
    /// # 返回
    /// - `Ok(true)`: 需要重新哈希
    /// - `Ok(false)`: 不需要重新哈希
    /// - `Err(DomainError)`: 检查过程中出错
    pub fn needs_rehash(hash: &str, target_cost: u32) -> DomainResult<bool> {
        // 简单的检查：如果哈希格式不正确，返回需要重新哈希
        if !Self::is_valid_bcrypt_hash(hash) {
            return Ok(true);
        }

        // 提取当前的 cost 值
        let current_cost = Self::extract_cost_from_hash(hash)?;

        // 如果 cost 值不同，需要重新哈希
        Ok(current_cost != target_cost)
    }

    /// 验证密码强度
    ///
    /// # 参数
    /// - `password`: 要验证的密码
    ///
    /// # 返回
    /// - `Ok(())`: 密码符合要求
    /// - `Err(DomainError)`: 密码不符合要求
    fn validate_password(password: &str) -> DomainResult<()> {
        if password.is_empty() {
            return Err(DomainError::validation_error("password", "密码不能为空"));
        }

        if password.len() < 6 {
            return Err(DomainError::validation_error(
                "password",
                "密码长度至少6个字符",
            ));
        }

        if password.len() > 128 {
            return Err(DomainError::validation_error(
                "password",
                "密码长度不能超过128个字符",
            ));
        }

        // 检查是否包含不可见字符（除了空格）
        if password.chars().any(|c| c.is_control() && c != ' ') {
            return Err(DomainError::validation_error(
                "password",
                "密码不能包含控制字符",
            ));
        }

        Ok(())
    }

    /// 检查是否为有效的 bcrypt 哈希
    fn is_valid_bcrypt_hash(hash: &str) -> bool {
        // bcrypt 哈希格式：$2a$10$... 或 $2b$10$... 或 $2y$10$...
        hash.starts_with("$2") && hash.len() == 60
    }

    /// 从 bcrypt 哈希中提取 cost 值
    fn extract_cost_from_hash(hash: &str) -> DomainResult<u32> {
        if !Self::is_valid_bcrypt_hash(hash) {
            return Err(DomainError::validation_error(
                "hash",
                "无效的 bcrypt 哈希格式",
            ));
        }

        // bcrypt 哈希格式：$2a$10$...
        // cost 在第二个 $ 符号后面
        let parts: Vec<&str> = hash.split('$').collect();
        if parts.len() < 4 {
            return Err(DomainError::validation_error(
                "hash",
                "无效的 bcrypt 哈希格式",
            ));
        }

        parts[2]
            .parse::<u32>()
            .map_err(|_| DomainError::validation_error("hash", "无法解析 bcrypt cost 值"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "test_password_123";
        let hash = PasswordService::hash_password(password).unwrap();

        // 验证哈希格式
        assert!(hash.starts_with("$2b$"));
        assert_eq!(hash.len(), 60);

        // 验证密码验证
        assert!(PasswordService::verify_password(password, &hash).unwrap());
        assert!(!PasswordService::verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_password_hashing_with_custom_cost() {
        let password = "test_password_123";
        let hash = PasswordService::hash_password_with_cost(password, 10).unwrap();

        // 验证哈希格式
        assert!(hash.starts_with("$2b$"));
        assert!(hash.contains("$10$"));

        // 验证密码验证
        assert!(PasswordService::verify_password(password, &hash).unwrap());
    }

    #[test]
    fn test_password_validation() {
        // 有效密码
        assert!(PasswordService::hash_password("password123").is_ok());
        assert!(PasswordService::hash_password("MySecureP@ssw0rd!").is_ok());
        assert!(PasswordService::hash_password("simple").is_ok()); // 6个字符，应该有效

        // 无效密码
        assert!(PasswordService::hash_password("").is_err()); // 空密码
        assert!(PasswordService::hash_password("123").is_err()); // 太短（3个字符）
        assert!(PasswordService::hash_password("12345").is_err()); // 太短（5个字符）
        assert!(PasswordService::hash_password(&"a".repeat(129)).is_err()); // 太长
    }

    #[test]
    fn test_invalid_cost_parameter() {
        let password = "test_password_123";

        // 无效的 cost 参数
        assert!(PasswordService::hash_password_with_cost(password, 9).is_err());
        assert!(PasswordService::hash_password_with_cost(password, 15).is_err());

        // 有效的 cost 参数
        assert!(PasswordService::hash_password_with_cost(password, 10).is_ok());
        assert!(PasswordService::hash_password_with_cost(password, 12).is_ok());
    }

    #[test]
    fn test_password_verification_with_wrong_hash() {
        let password = "test_password_123";

        // 使用无效的哈希格式
        assert!(PasswordService::verify_password(password, "invalid_hash").is_err());
        assert!(PasswordService::verify_password(password, "").is_err());
    }

    #[test]
    fn test_needs_rehash() {
        let password = "test_password_123";
        let hash_cost_10 = PasswordService::hash_password_with_cost(password, 10).unwrap();

        // 相同 cost，不需要重新哈希
        assert!(!PasswordService::needs_rehash(&hash_cost_10, 10).unwrap());

        // 不同 cost，需要重新哈希
        assert!(PasswordService::needs_rehash(&hash_cost_10, 12).unwrap());

        // 无效哈希，需要重新哈希
        assert!(PasswordService::needs_rehash("invalid_hash", 12).unwrap());
    }

    #[test]
    fn test_bcrypt_hash_validation() {
        // 生成真实的 bcrypt 哈希进行测试
        let password = "test_password";
        let hash = PasswordService::hash_password(password).unwrap();
        assert!(PasswordService::is_valid_bcrypt_hash(&hash));

        // 无效的 bcrypt 哈希
        assert!(!PasswordService::is_valid_bcrypt_hash("invalid_hash"));
        assert!(!PasswordService::is_valid_bcrypt_hash("$1$invalid"));
        assert!(!PasswordService::is_valid_bcrypt_hash("$2b$12$short"));
    }

    #[test]
    fn test_extract_cost_from_hash() {
        // 生成真实的 bcrypt 哈希进行测试
        let hash_cost_10 = PasswordService::hash_password_with_cost("test123", 10).unwrap();
        let hash_cost_12 = PasswordService::hash_password_with_cost("test123", 12).unwrap();

        assert_eq!(
            PasswordService::extract_cost_from_hash(&hash_cost_10).unwrap(),
            10
        );
        assert_eq!(
            PasswordService::extract_cost_from_hash(&hash_cost_12).unwrap(),
            12
        );

        // 无效哈希
        assert!(PasswordService::extract_cost_from_hash("invalid").is_err());
    }

    #[test]
    fn test_password_with_special_characters() {
        let passwords = vec![
            "P@ssw0rd!",
            "密码123",
            "пароль",
            "🔒secure🔑",
            "test with spaces",
        ];

        for password in passwords {
            let hash = PasswordService::hash_password(password).unwrap();
            assert!(PasswordService::verify_password(password, &hash).unwrap());
        }
    }

    #[test]
    fn test_performance_requirements() {
        let password = "test_password_123";

        // 测试单次哈希操作时间
        let start = std::time::Instant::now();
        let hash = PasswordService::hash_password(password).unwrap();
        let single_hash_duration = start.elapsed();

        // 单次哈希操作应该在合理时间内完成（bcrypt 通常需要几十毫秒到几秒）
        assert!(
            single_hash_duration.as_secs() < 5,
            "单次哈希操作耗时过长: {:?}",
            single_hash_duration
        );

        // 测试单次验证性能
        let start = std::time::Instant::now();
        assert!(PasswordService::verify_password(password, &hash).unwrap());
        let single_verify_duration = start.elapsed();

        // 单次验证应该比哈希快一些，但仍需要时间
        assert!(
            single_verify_duration.as_secs() < 3,
            "单次验证操作耗时过长: {:?}",
            single_verify_duration
        );

        // 测试错误密码验证（应该也在合理时间内完成）
        let start = std::time::Instant::now();
        assert!(!PasswordService::verify_password("wrong_password", &hash).unwrap());
        let wrong_verify_duration = start.elapsed();

        assert!(
            wrong_verify_duration.as_secs() < 3,
            "错误密码验证耗时过长: {:?}",
            wrong_verify_duration
        );
    }
}
