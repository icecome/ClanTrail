//! Tomb Keeper 核心库
//!
//! 离线优先的墓地信息管理核心：数据模型、SQLite 持久化、搜索。
//! 该 crate 不依赖任何 UI 框架，可被 Tauri（移动端）与 Axum（Web 服务端）复用。

pub mod db;
pub mod error;
pub mod models;

pub use db::TombKeeperDb;
pub use error::{AppError, Result};
pub use models::*;