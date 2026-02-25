//! EnvCLI - 跨平台环境变量管理工具
//!
//! 重构后的代码结构，遵循 Clean Architecture 原则

// 领域层
pub mod domain;

// 应用层
pub mod application;

// 基础设施层
pub mod infrastructure;

// 命令层
pub mod commands;

// CLI 定义
pub mod cli;

// 应用程序容器
pub mod app;

// 重新导出常用类型
pub use domain::{DomainError, EnvSource, EnvVar, OutputFormat, Result};
