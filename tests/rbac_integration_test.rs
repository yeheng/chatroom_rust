use std::sync::Arc;

use application::{
    clock::SystemClock,
    password::BcryptPasswordHasher,
    services::{
        chat_service::{ChatService, ChatServiceDependencies, CreateRoomRequest},
        user_service::{RegisterUserRequest, UserService, UserServiceDependencies},
    },
};
use domain::{ChatRoomVisibility, RoomRole, User, UserId};
use infrastructure::{repository::PgStorage, OnlineStatsSummary, RedisPresenceManager};
use sqlx::PgPool;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

/// RBAC 集成测试 - 验证基于角色的访问控制
/// 这个测试验证了：
/// 1. 超级用户权限 (is_superuser)
/// 2. 房间角色权限 (Owner, Admin, Member)
/// 3. 权限检查逻辑正确性
#[tokio::test]
async fn rbac_permission_system_works() {
    // 启动 PostgreSQL 容器进行测试
    let container: ContainerAsync<Postgres> =
        Postgres::default().start().await.expect("启动 postgres");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

    // 创建数据库连接池
    let pool = PgPool::connect(&database_url)
        .await
        .expect("连接到测试数据库");

    // 运行迁移
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("运行迁移");

    // 创建存储和服务
    let storage = PgStorage::new(pool.clone());
    let password_hasher = Arc::new(BcryptPasswordHasher::new(4));
    let clock = Arc::new(SystemClock);

    // 模拟广播器
    #[derive(Clone)]
    struct MockBroadcaster;

    impl application::broadcaster::MessageBroadcaster for MockBroadcaster {
        async fn broadcast(
            &self,
            _message: application::broadcaster::MessageBroadcast,
        ) -> Result<(), application::error::ApplicationError> {
            Ok(())
        }
    }

    // 创建用户服务
    let user_service = UserService::new(UserServiceDependencies {
        user_repository: storage.user_repository.clone(),
        password_hasher: password_hasher.clone(),
        clock: clock.clone(),
        presence_manager: Arc::new(RedisPresenceManager::new(
            Arc::new(redis::Client::open("redis://127.0.0.1:6379").unwrap()),
            "test_presence",
            "test_events",
        )),
    });

    // 创建聊天服务
    let chat_service = ChatService::new(ChatServiceDependencies {
        room_repository: storage.room_repository.clone(),
        member_repository: storage.member_repository.clone(),
        message_repository: storage.message_repository.clone(),
        password_hasher: password_hasher.clone(),
        clock: clock.clone(),
        broadcaster: Arc::new(MockBroadcaster),
    });

    // 1. 创建测试用户
    println!("🔧 创建测试用户...");

    // 创建普通用户
    let regular_user = user_service
        .register(RegisterUserRequest {
            username: "regular_user".to_string(),
            email: "regular@example.com".to_string(),
            password: "password123".to_string(),
        })
        .await
        .expect("创建普通用户");

    // 创建超级用户
    let mut superuser = user_service
        .register(RegisterUserRequest {
            username: "superuser".to_string(),
            email: "super@example.com".to_string(),
            password: "password123".to_string(),
        })
        .await
        .expect("创建超级用户");

    // 提升为超级用户
    superuser.grant_superuser(clock.now());
    let superuser = storage
        .user_repository
        .update(superuser)
        .await
        .expect("更新超级用户状态");

    println!("✅ 用户创建完成 - 普通用户: {}, 超级用户: {}",
             regular_user.id, superuser.id);

    // 2. 测试超级用户权限
    println!("🔧 测试超级用户权限...");

    assert!(!regular_user.is_system_admin(), "普通用户不应该是系统管理员");
    assert!(superuser.is_system_admin(), "超级用户应该是系统管理员");

    println!("✅ 超级用户权限验证通过");

    // 3. 创建房间测试房间角色权限
    println!("🔧 创建测试房间...");

    let room = chat_service
        .create_room(CreateRoomRequest {
            name: "测试房间".to_string(),
            owner_id: regular_user.id.into(),
            visibility: ChatRoomVisibility::Public,
            password: None,
        })
        .await
        .expect("创建房间");

    println!("✅ 房间创建完成: {}", room.id);

    // 4. 测试房间角色权限
    println!("🔧 测试房间角色权限...");

    // 获取房间所有者角色
    let owner_role = chat_service
        .get_user_role_in_room(room.id, regular_user.id)
        .await
        .expect("获取所有者角色")
        .expect("所有者应该有角色");

    assert_eq!(owner_role, RoomRole::Owner, "创建者应该是房间所有者");
    assert!(owner_role.has_admin_access(), "所有者应该有管理员权限");
    assert!(owner_role.is_owner(), "所有者角色检查");
    assert!(owner_role.can_manage_members(), "所有者应该能管理成员");
    assert!(owner_role.can_delete_messages(), "所有者应该能删除消息");

    // 超级用户不在房间，应该没有角色
    let superuser_role = chat_service
        .get_user_role_in_room(room.id, superuser.id)
        .await
        .expect("获取超级用户角色");

    assert!(superuser_role.is_none(), "超级用户不在房间时应该没有角色");

    println!("✅ 房间角色权限验证通过");

    // 5. 测试应用层的统一权限检查逻辑
    println!("🔧 测试权限检查逻辑...");

    // 直接测试应用层的权限检查方法 - Linus式：直接测试真实逻辑
    async fn test_admin_access(
        chat_service: &ChatService,
        user_id: Uuid,
        room_id: Option<Uuid>,
    ) -> Result<(), String> {
        use domain::{RoomId, UserId};

        let user_id = UserId::from(user_id);
        let room_id = room_id.map(RoomId::from);

        chat_service
            .check_admin_access(user_id, room_id)
            .await
            .map_err(|err| err.to_string())
    }

            // 获取用户在房间中的角色
            let role = chat_service
                .get_user_role_in_room(room_id, user_id)
                .await
                .map_err(|err| format!("获取用户角色失败: {}", err))?;

            match role {
                Some(room_role) => {
                    // 只有房间 Owner 或 Admin 才能访问房间统计
                    if room_role.has_admin_access() {
                        Ok(())
                    } else {
                        Err("只有房间所有者和管理员才能访问房间统计".to_string())
                    }
                }
                None => Err("用户不是此房间的成员".to_string()),
            }
        } else {
            // 全局统计只有系统管理员可以访问
    // 测试超级用户访问全局统计
    let result = test_admin_access(&chat_service, superuser.id.into(), None).await;
    assert!(result.is_ok(), "超级用户应该能访问全局统计");

    // 测试普通用户访问全局统计（应该失败）
    let result = test_admin_access(&chat_service, regular_user.id.into(), None).await;
    assert!(result.is_err(), "普通用户不应该能访问全局统计");

    // 测试房间所有者访问房间统计
    let result = test_admin_access(
        &chat_service,
        regular_user.id.into(),
        Some(room.id.into()),
    )
    .await;
    assert!(result.is_ok(), "房间所有者应该能访问房间统计");

    // 测试超级用户访问房间统计
    let result = test_admin_access(
        &chat_service,
        superuser.id.into(),
        Some(room.id.into()),
    )
    .await;
    assert!(result.is_ok(), "超级用户应该能访问任何房间统计");

    println!("✅ 权限检查逻辑验证通过");

    // 6. 测试数据持久化
    println!("🔧 测试数据持久化...");

    // 重新从数据库加载用户，验证 is_superuser 字段正确保存
    let loaded_superuser = storage
        .user_repository
        .find_by_id(superuser.id)
        .await
        .expect("加载超级用户")
        .expect("超级用户应该存在");

    assert!(loaded_superuser.is_system_admin(), "加载的超级用户应该保持管理员状态");

    let loaded_regular_user = storage
        .user_repository
        .find_by_id(regular_user.id)
        .await
        .expect("加载普通用户")
        .expect("普通用户应该存在");

    assert!(!loaded_regular_user.is_system_admin(), "加载的普通用户应该不是管理员");

    println!("✅ 数据持久化验证通过");

    println!("🎉 RBAC 系统集成测试完全通过！");
}

/// 测试 RoomRole 权限方法
#[test]
fn room_role_permissions_work() {
    // 测试 Owner 权限
    let owner = RoomRole::Owner;
    assert!(owner.has_admin_access());
    assert!(owner.is_owner());
    assert!(owner.can_manage_members());
    assert!(owner.can_delete_messages());

    // 测试 Admin 权限
    let admin = RoomRole::Admin;
    assert!(admin.has_admin_access());
    assert!(!admin.is_owner());
    assert!(admin.can_manage_members());
    assert!(admin.can_delete_messages());

    // 测试 Member 权限
    let member = RoomRole::Member;
    assert!(!member.has_admin_access());
    assert!(!member.is_owner());
    assert!(!member.can_manage_members());
    assert!(!member.can_delete_messages());
}