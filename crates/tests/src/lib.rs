//! 端到端测试框架
//! 
//! 提供测试环境管理、测试数据工厂、WebSocket测试客户端等工具

pub mod test_environment;
pub mod test_data_factory;
pub mod websocket_client;
pub mod performance_tests;
pub mod test_utils;

// 重新导出常用类型
pub use test_environment::*;
pub use test_data_factory::*;
pub use websocket_client::*;
pub use test_utils::*;