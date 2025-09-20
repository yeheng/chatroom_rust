//! 应用层
//!
//! 处理用例和应用服务。采用 CQRS（命令查询职责分离）架构模式。

pub mod cqrs;
pub mod errors;
pub mod services;

pub use cqrs::*;
pub use errors::*;
pub use services::*;
