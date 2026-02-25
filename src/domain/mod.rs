//! Domain Layer - 核心业务逻辑
//!
//! 包含：
//! - models: 领域实体
//! - repositories: 存储接口（输出端口）
//! - error: 领域错误类型

pub mod error;
pub mod models;
pub mod repositories;

pub use error::{DomainError, Result};
pub use models::{EnvSource, EnvVar, OutputFormat};
pub use repositories::{EnvRepository, RepositoryFactory};
