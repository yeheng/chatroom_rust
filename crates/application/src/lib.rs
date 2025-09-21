//! 应用层
//!
//! 处理用例和应用服务。采用 CQRS（命令查询职责分离）架构模式。

pub mod cqrs;
pub mod errors;
pub mod services;

// 避免重复从 cqrs 与 services 同时通配导出，导致同名类型二次导出
// cqrs 模块已对外导出其下的 services 中需要的类型
pub use cqrs::*;
pub use errors::*;
