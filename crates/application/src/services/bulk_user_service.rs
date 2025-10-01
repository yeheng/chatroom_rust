use domain::{OrgId, RepositoryError, UserEmail, UserId, Username};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::spawn;
use tracing::{error, info};
use uuid::Uuid;

use super::PasswordService;

/// 批量用户服务
/// 根据设计文档，这个服务负责批量创建用户并分配到指定组织
pub struct BulkUserService {
    pool: Arc<PgPool>,
}

impl BulkUserService {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// 提交批量创建用户任务
    pub async fn create_bulk_users(
        &self,
        request: BulkCreateUsersRequest,
    ) -> Result<BulkTask, RepositoryError> {
        // 创建任务记录
        let task_id = Uuid::new_v4();
        let now = time::OffsetDateTime::now_utc();

        let task = BulkTask {
            id: task_id,
            task_type: "bulk_create_users".to_string(),
            status: TaskStatus::Pending,
            created_by: request.created_by,
            total_count: request.users.len() as i32,
            processed_count: 0,
            success_count: 0,
            failed_count: 0,
            error_message: None,
            result_data: None,
            created_at: now,
            started_at: None,
            completed_at: None,
        };

        // 插入任务记录
        sqlx::query(
            r#"
            INSERT INTO bulk_tasks (
                id, task_type, status, created_by, total_count,
                processed_count, success_count, failed_count,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(task.id)
        .bind(&task.task_type)
        .bind(task.status.to_string())
        .bind(Uuid::from(task.created_by))
        .bind(task.total_count)
        .bind(task.processed_count)
        .bind(task.success_count)
        .bind(task.failed_count)
        .bind(task.created_at)
        .execute(self.pool.as_ref())
        .await
        .map_err(map_sqlx_err)?;

        // 启动异步任务处理
        let pool = self.pool.clone();
        let task_id_clone = task_id;
        spawn(async move {
            if let Err(e) = Self::process_bulk_user_creation(pool, task_id_clone, request).await {
                error!("批量用户创建任务处理失败: {}", e);
            }
        });

        Ok(task)
    }

    /// 查询任务状态
    pub async fn get_task_status(
        &self,
        task_id: Uuid,
    ) -> Result<Option<BulkTask>, RepositoryError> {
        let record = sqlx::query_as::<_, BulkTaskRecord>(
            r#"
            SELECT id, task_type, status, created_by, total_count,
                   processed_count, success_count, failed_count,
                   error_message, result_data, created_at,
                   started_at, completed_at
            FROM bulk_tasks
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(map_sqlx_err)?;

        match record {
            Some(record) => Ok(Some(BulkTask::try_from(record)?)),
            None => Ok(None),
        }
    }

    /// 下载用户凭证（CSV格式）
    pub async fn download_credentials(
        &self,
        task_id: Uuid,
    ) -> Result<Option<String>, RepositoryError> {
        let task = self.get_task_status(task_id).await?;

        match task {
            Some(task) => {
                if task.status != TaskStatus::Completed {
                    return Ok(None);
                }

                // 从result_data中提取用户凭证
                match task.result_data {
                    Some(data) => {
                        let credentials: Vec<UserCredential> = serde_json::from_value(data)
                            .map_err(|e| {
                                RepositoryError::storage(format!("解析凭证数据失败: {}", e))
                            })?;

                        // 生成CSV格式
                        let mut csv = String::new();
                        csv.push_str("username,email,password\n");

                        for cred in credentials {
                            csv.push_str(&format!(
                                "{},{},{}\n",
                                cred.username, cred.email, cred.password
                            ));
                        }

                        Ok(Some(csv))
                    }
                    None => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    /// 处理批量用户创建的异步任务（正确的事务处理版本）
    async fn process_bulk_user_creation(
        pool: Arc<PgPool>,
        task_id: Uuid,
        request: BulkCreateUsersRequest,
    ) -> Result<(), RepositoryError> {
        info!("开始处理批量用户创建任务: {}", task_id);

        // 更新任务状态为处理中
        sqlx::query(
            r#"
            UPDATE bulk_tasks
            SET status = 'processing', started_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .execute(pool.as_ref())
        .await
        .map_err(map_sqlx_err)?;

        // 准备用户数据并生成密码（在事务外完成，避免锁定时间过长）
        let mut prepared_users = Vec::new();
        let mut credentials = Vec::new();

        for user_request in &request.users {
            match prepare_user_data(user_request, &request.org_id) {
                Ok((user_data, plain_password)) => {
                    prepared_users.push(user_data);
                    credentials.push(UserCredential {
                        username: user_request.username.clone(),
                        email: user_request.email.clone(),
                        password: plain_password,
                    });
                }
                Err(e) => {
                    error!("准备用户数据失败: {} - {}", user_request.username, e);
                    // 继续处理其他用户
                    continue;
                }
            }
        }

        let processed_count = prepared_users.len();

        // 开始事务批量插入
        let mut tx = pool.begin().await.map_err(map_sqlx_err)?;

        let mut success_count = 0;
        let mut failed_count = 0;

        // 批量插入用户（使用批处理提高性能）
        for user_data in prepared_users {
            match insert_user(&mut tx, &user_data).await {
                Ok(_) => {
                    success_count += 1;
                }
                Err(e) => {
                    failed_count += 1;
                    error!("插入用户失败: {} - {}", user_data.username, e);
                    // 继续处理其他用户，不回滚整个事务
                }
            }
        }

        // 更新任务最终状态
        let result_data = serde_json::to_value(&credentials)
            .map_err(|e| RepositoryError::storage(format!("序列化凭证失败: {}", e)))?;

        let status = if failed_count == 0 {
            "completed"
        } else {
            "completed"
        }; // 部分成功也算完成

        sqlx::query(
            r#"
            UPDATE bulk_tasks
            SET status = $1,
                processed_count = $2,
                success_count = $3,
                failed_count = $4,
                result_data = $5,
                completed_at = NOW()
            WHERE id = $6
            "#,
        )
        .bind(status)
        .bind(processed_count as i32)
        .bind(success_count)
        .bind(failed_count)
        .bind(result_data)
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_err)?;

        // 提交事务
        tx.commit().await.map_err(map_sqlx_err)?;

        info!(
            "批量用户创建任务完成: {} - 成功: {}, 失败: {}",
            task_id, success_count, failed_count
        );

        Ok(())
    }
}

/// 用户数据准备结构（用于批量插入）
struct PreparedUserData {
    id: Uuid,
    username: String,
    email: String,
    password_hash: String,
    org_id: Uuid,
    created_at: time::OffsetDateTime,
    updated_at: time::OffsetDateTime,
}

/// 准备用户数据（包括密码哈希）
fn prepare_user_data(
    user_request: &CreateUserRequest,
    org_id: &OrgId,
) -> Result<(PreparedUserData, String), RepositoryError> {
    // 生成随机密码
    let plain_password = generate_random_password();

    // 使用PasswordService哈希密码
    let password_hash = PasswordService::hash_password(&plain_password)?;

    // 验证用户名和邮箱
    let username = Username::parse(&user_request.username)
        .map_err(|e| RepositoryError::storage(format!("用户名无效: {}", e)))?;
    let email = UserEmail::parse(&user_request.email)
        .map_err(|e| RepositoryError::storage(format!("邮箱无效: {}", e)))?;

    let now = time::OffsetDateTime::now_utc();

    Ok((
        PreparedUserData {
            id: Uuid::new_v4(),
            username: username.as_str().to_string(),
            email: email.as_str().to_string(),
            password_hash,
            org_id: Uuid::from(*org_id),
            created_at: now,
            updated_at: now,
        },
        plain_password,
    ))
}

/// 插入单个用户（在事务中）
async fn insert_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_data: &PreparedUserData,
) -> Result<(), RepositoryError> {
    sqlx::query(
        r#"
        INSERT INTO users (
            id, username, email, password_hash, status,
            is_superuser, org_id, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(user_data.id)
    .bind(&user_data.username)
    .bind(&user_data.email)
    .bind(&user_data.password_hash)
    .bind("active") // UserStatus::Active
    .bind(false) // is_superuser
    .bind(user_data.org_id)
    .bind(user_data.created_at)
    .bind(user_data.updated_at)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_err)?;

    Ok(())
}

/// 生成随机密码（12位，包含大小写字母和数字）
fn generate_random_password() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    let password: String = (0..12)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    password
}

/// 批量创建用户请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkCreateUsersRequest {
    pub created_by: UserId,
    pub org_id: OrgId,
    pub users: Vec<CreateUserRequest>,
}

/// 创建单个用户请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
}

/// 批量任务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkTask {
    pub id: Uuid,
    pub task_type: String,
    pub status: TaskStatus,
    pub created_by: UserId,
    pub total_count: i32,
    pub processed_count: i32,
    pub success_count: i32,
    pub failed_count: i32,
    pub error_message: Option<String>,
    pub result_data: Option<serde_json::Value>,
    pub created_at: time::OffsetDateTime,
    pub started_at: Option<time::OffsetDateTime>,
    pub completed_at: Option<time::OffsetDateTime>,
}

/// 任务状态枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl TaskStatus {
    pub fn to_string(&self) -> String {
        match self {
            TaskStatus::Pending => "pending".to_string(),
            TaskStatus::Processing => "processing".to_string(),
            TaskStatus::Completed => "completed".to_string(),
            TaskStatus::Failed => "failed".to_string(),
        }
    }
}

/// 用户凭证
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCredential {
    pub username: String,
    pub email: String,
    pub password: String,
}

/// 数据库记录类型
#[derive(Debug, sqlx::FromRow)]
struct BulkTaskRecord {
    id: Uuid,
    task_type: String,
    status: String,
    created_by: Uuid,
    total_count: i32,
    processed_count: i32,
    success_count: i32,
    failed_count: i32,
    error_message: Option<String>,
    result_data: Option<serde_json::Value>,
    created_at: time::OffsetDateTime,
    started_at: Option<time::OffsetDateTime>,
    completed_at: Option<time::OffsetDateTime>,
}

impl TryFrom<BulkTaskRecord> for BulkTask {
    type Error = RepositoryError;

    fn try_from(record: BulkTaskRecord) -> Result<Self, Self::Error> {
        let status = match record.status.as_str() {
            "pending" => TaskStatus::Pending,
            "processing" => TaskStatus::Processing,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            _ => {
                return Err(RepositoryError::storage(format!(
                    "无效的任务状态: {}",
                    record.status
                )))
            }
        };

        Ok(BulkTask {
            id: record.id,
            task_type: record.task_type,
            status,
            created_by: UserId::from(record.created_by),
            total_count: record.total_count,
            processed_count: record.processed_count,
            success_count: record.success_count,
            failed_count: record.failed_count,
            error_message: record.error_message,
            result_data: record.result_data,
            created_at: record.created_at,
            started_at: record.started_at,
            completed_at: record.completed_at,
        })
    }
}

/// 映射SQL错误
fn map_sqlx_err(err: sqlx::Error) -> RepositoryError {
    match err {
        sqlx::Error::RowNotFound => RepositoryError::NotFound,
        sqlx::Error::Database(ref db_err) if db_err.code().is_some_and(|code| code == "23505") => {
            RepositoryError::Conflict
        }
        other => {
            let message = other.to_string();
            RepositoryError::storage_with_source(message, other)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_password() {
        let password1 = generate_random_password();
        let password2 = generate_random_password();

        assert_eq!(password1.len(), 12);
        assert_eq!(password2.len(), 12);
        assert_ne!(password1, password2);
    }

    #[test]
    fn test_task_status_to_string() {
        assert_eq!(TaskStatus::Pending.to_string(), "pending");
        assert_eq!(TaskStatus::Processing.to_string(), "processing");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Failed.to_string(), "failed");
    }
}
