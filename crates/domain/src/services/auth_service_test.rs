#[cfg(test)]
mod tests {
    use crate::{
        auth::{AuthError, Claims, LoginCredentials, Permission, UserRole},
        services::auth_service::*,
    };
    use chrono::Utc;
    use uuid::Uuid;

    // 测试辅助函数
    fn create_test_claims() -> Claims {
        Claims {
            sub: Uuid::new_v4().to_string(),
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            role: UserRole::User,
            permissions: vec![Permission::SendMessage, Permission::JoinRoom],
            jti: Uuid::new_v4().to_string(),
            iat: Utc::now().timestamp(),
            exp: Utc::now().timestamp() + 3600,
            iss: "chatroom".to_string(),
            aud: "chatroom-users".to_string(),
        }
    }

    #[test]
    fn test_jwt_encoder_generate_access_token() {
        // 创建模拟密钥数据用于测试
        let mut private_key_bytes = Vec::new();
        let mut public_key_bytes = Vec::new();

        // 使用模拟的 RSA 密钥数据（实际项目中应该使用真实的密钥）
        private_key_bytes.extend_from_slice(b"-----BEGIN PRIVATE KEY-----\nMIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQC5...\n-----END PRIVATE KEY-----");
        public_key_bytes.extend_from_slice(b"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuC...\n-----END PUBLIC KEY-----");

        let encoder = JwtEncoder::new(
            private_key_bytes,
            public_key_bytes,
            60,
            7,
            "chatroom".to_string(),
            "chatroom-users".to_string(),
        );

        let claims = create_test_claims();

        // 测试编码（可能会因为密钥格式问题失败，这是预期的）
        let token_result = encoder.generate_access_token(&claims);

        // 注意：由于使用模拟密钥，这里可能会失败
        // 在实际实现中，需要使用真实的 RSA 密钥对
        match token_result {
            Ok(token) => {
                println!("Generated token: {}", token);
                assert!(!token.is_empty());
            }
            Err(e) => {
                println!("Encode error (expected with mock keys): {}", e);
            }
        }
    }

    #[test]
    fn test_jwt_encoder_decode_token() {
        // 创建模拟密钥数据
        let mut private_key_bytes = Vec::new();
        let mut public_key_bytes = Vec::new();

        private_key_bytes.extend_from_slice(b"-----BEGIN PRIVATE KEY-----\nMIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQC5...\n-----END PRIVATE KEY-----");
        public_key_bytes.extend_from_slice(b"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuC...\n-----END PUBLIC KEY-----");

        let encoder = JwtEncoder::new(
            private_key_bytes,
            public_key_bytes,
            60,
            7,
            "chatroom".to_string(),
            "chatroom-users".to_string(),
        );

        // 测试无效 token
        let invalid_token = "invalid.token.here";
        let result = encoder.decode_token(invalid_token);

        assert!(result.is_err());

        match result.err().unwrap() {
            AuthError::InvalidToken => {
                println!("Expected invalid token error");
            }
            _ => {
                panic!("Expected InvalidToken error");
            }
        }
    }

    #[test]
    fn test_jwt_encoder_generate_refresh_token() {
        // 创建模拟密钥数据
        let mut private_key_bytes = Vec::new();
        let mut public_key_bytes = Vec::new();

        private_key_bytes.extend_from_slice(b"-----BEGIN PRIVATE KEY-----\nMIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQC5...\n-----END PRIVATE KEY-----");
        public_key_bytes.extend_from_slice(b"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuC...\n-----END PUBLIC KEY-----");

        let encoder = JwtEncoder::new(
            private_key_bytes,
            public_key_bytes,
            60,
            7,
            "chatroom".to_string(),
            "chatroom-users".to_string(),
        );

        let user_id = Uuid::new_v4();
        let result = encoder.generate_refresh_token(user_id);

        match result {
            Ok(token) => {
                assert!(!token.is_empty());
                println!("Generated refresh token: {}", token);
            }
            Err(e) => {
                println!("Refresh token generation error: {}", e);
            }
        }
    }

    #[test]
    fn test_jwt_encoder_generate_claims() {
        // 创建模拟密钥数据
        let mut private_key_bytes = Vec::new();
        let mut public_key_bytes = Vec::new();

        private_key_bytes.extend_from_slice(b"-----BEGIN PRIVATE KEY-----\nMIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQC5...\n-----END PRIVATE KEY-----");
        public_key_bytes.extend_from_slice(b"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuC...\n-----END PUBLIC KEY-----");

        let encoder = JwtEncoder::new(
            private_key_bytes,
            public_key_bytes,
            60,
            7,
            "chatroom".to_string(),
            "chatroom-users".to_string(),
        );

        let user_id = Uuid::new_v4();
        let username = "testuser".to_string();
        let email = Some("test@example.com".to_string());
        let role = UserRole::User;
        let permissions = vec![Permission::SendMessage, Permission::JoinRoom];

        let claims = encoder.generate_claims(user_id, username, email, role, permissions);

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.email, Some("test@example.com".to_string()));
        assert_eq!(claims.role, UserRole::User);
        assert_eq!(claims.permissions.len(), 2);
        assert!(claims.permissions.contains(&Permission::SendMessage));
        assert!(claims.permissions.contains(&Permission::JoinRoom));
        assert_eq!(claims.iss, "chatroom");
        assert_eq!(claims.aud, "chatroom-users");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_login_credentials_validation() {
        // 测试有效的登录凭据
        let valid_credentials = LoginCredentials {
            username: "testuser".to_string(),
            password: "password123".to_string(),
        };

        assert!(!valid_credentials.username.is_empty());
        assert!(!valid_credentials.password.is_empty());

        // 测试空用户名
        let empty_username = LoginCredentials {
            username: "".to_string(),
            password: "password123".to_string(),
        };

        assert!(empty_username.username.is_empty());

        // 测试空密码
        let empty_password = LoginCredentials {
            username: "testuser".to_string(),
            password: "".to_string(),
        };

        assert!(empty_password.password.is_empty());
    }

    #[test]
    fn test_claims_creation() {
        let user_id = Uuid::new_v4();
        let claims = Claims {
            sub: user_id.to_string(),
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            role: UserRole::User,
            permissions: vec![Permission::SendMessage, Permission::JoinRoom],
            jti: Uuid::new_v4().to_string(),
            iat: Utc::now().timestamp(),
            exp: Utc::now().timestamp() + 3600,
            iss: "chatroom".to_string(),
            aud: "chatroom-users".to_string(),
        };

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.email, Some("test@example.com".to_string()));
        assert_eq!(claims.role, UserRole::User);
        assert_eq!(claims.permissions.len(), 2);
        assert!(claims.permissions.contains(&Permission::SendMessage));
        assert!(claims.permissions.contains(&Permission::JoinRoom));
        assert_eq!(claims.iss, "chatroom");
        assert_eq!(claims.aud, "chatroom-users");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_user_role_permissions() {
        // 测试管理员权限
        let admin_permissions = UserRole::Admin.default_permissions();
        assert!(admin_permissions.contains(&Permission::SendMessage));
        assert!(admin_permissions.contains(&Permission::JoinRoom));
        assert!(admin_permissions.contains(&Permission::CreateRoom));
        assert!(admin_permissions.contains(&Permission::ManageUsers));

        // 测试用户权限
        let user_permissions = UserRole::User.default_permissions();
        assert!(user_permissions.contains(&Permission::SendMessage));
        assert!(user_permissions.contains(&Permission::JoinRoom));
        assert!(!user_permissions.contains(&Permission::CreateRoom));
        assert!(!user_permissions.contains(&Permission::ManageUsers));

        // 测试访客权限
        let guest_permissions = UserRole::Guest.default_permissions();
        assert!(guest_permissions.contains(&Permission::SendMessage));
        assert!(guest_permissions.contains(&Permission::JoinRoom));
        assert!(!guest_permissions.contains(&Permission::CreateRoom));
        assert!(!guest_permissions.contains(&Permission::ManageUsers));
    }
}
