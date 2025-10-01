use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use domain::RepositoryError;

/// 密码服务 - 负责密码的哈希和验证
/// 使用Argon2算法，这是目前最安全的密码哈希算法之一
pub struct PasswordService;

impl PasswordService {
    /// 哈希密码 - 使用Argon2算法
    ///
    /// # 安全性
    /// - 使用随机salt
    /// - 自动处理迭代次数和内存成本
    /// - 生成的哈希包含所有必要的参数信息
    pub fn hash_password(password: &str) -> Result<String, RepositoryError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| RepositoryError::storage(format!("密码哈希失败: {}", e)))?
            .to_string();

        Ok(password_hash)
    }

    /// 验证密码 - 对比明文密码和哈希值
    ///
    /// # 参数
    /// - `password`: 用户输入的明文密码
    /// - `password_hash`: 数据库中存储的哈希值
    ///
    /// # 返回
    /// - `Ok(true)`: 密码正确
    /// - `Ok(false)`: 密码错误
    /// - `Err(_)`: 哈希格式错误或其他错误
    pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, RepositoryError> {
        let parsed_hash = PasswordHash::new(password_hash)
            .map_err(|e| RepositoryError::storage(format!("密码哈希格式错误: {}", e)))?;

        let argon2 = Argon2::default();

        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(_) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(RepositoryError::storage(format!("密码验证失败: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let password = "test_password_123";

        // 哈希密码
        let hash = PasswordService::hash_password(password).unwrap();

        // 验证正确的密码
        assert!(PasswordService::verify_password(password, &hash).unwrap());

        // 验证错误的密码
        assert!(!PasswordService::verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_hash_generates_different_salts() {
        let password = "same_password";

        let hash1 = PasswordService::hash_password(password).unwrap();
        let hash2 = PasswordService::hash_password(password).unwrap();

        // 即使密码相同，由于salt不同，哈希值也应该不同
        assert_ne!(hash1, hash2);

        // 但两个哈希都应该能验证原密码
        assert!(PasswordService::verify_password(password, &hash1).unwrap());
        assert!(PasswordService::verify_password(password, &hash2).unwrap());
    }
}
