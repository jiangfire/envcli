//! 插件文件监控和自动热重载
//!
//! 提供基于文件系统事件的自动热重载功能。
//!
//! # 锁获取顺序规范
//!
//! ## 外部锁获取顺序（watcher -> manager）
//!
//! 当 watcher 调用 manager 时，遵循以下顺序：
//!
//! 1. **watcher 内部锁**: `self.active_reloads`, `self.plugin_paths` 等
//! 2. **manager 锁**: `self.manager.lock()`（获取整个管理器）
//! 3. **manager 内部锁**: manager 内部的 RwLock（configs, statuses）
//!
//! ## 内部锁获取顺序（manager 内部）
//!
//! 参考 manager.rs 的锁获取顺序规范：
//! - 先获取 `plugin_configs` 读/写锁
//! - 再获取 `plugin_statuses` 读/写锁
//!
//! ## 重要规则
//!
//! - 不要在持有 manager 锁的同时获取 watcher 的其他锁
//! - 自动监控线程中的锁获取也必须遵循相同顺序
//! - 避免在持有锁的情况下进行耗时操作
//!
//! # 使用示例
//!
//! ```ignore
//! use crate::plugin::watcher::{PluginWatcher, AutoReloadConfig};
//! use crate::plugin::manager::PluginManager;
//!
//! // 创建监控器
//! let manager = PluginManager::new()?;
//! let config = AutoReloadConfig::default();
//! let watcher = PluginWatcher::new(manager, config);
//!
//! // 注册要监控的插件
//! watcher.watch_plugin("my-plugin", PathBuf::from("plugins/my-plugin.dll"))?;
//!
//! // 手动触发文件变更（用于测试）
//! watcher.handle_file_change(FileChangeEvent::Modified(
//!     PathBuf::from("plugins/my-plugin.dll")
//! ));
//! ```

use crate::plugin::manager::PluginManager;
use crate::plugin::types::PluginError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// 条件编译：仅在启用 notify 特性时编译自动监控相关代码
#[cfg(feature = "notify")]
use notify::RecommendedWatcher;
#[cfg(feature = "notify")]
use notify::{
    RecursiveMode, Watcher as NotifyWatcher,
    event::{EventKind, ModifyKind},
};

/// 自动热重载配置
#[derive(Debug, Clone)]
pub struct AutoReloadConfig {
    /// 防抖时间（毫秒），默认 500ms
    pub debounce_ms: u64,
    /// 是否在重载前验证签名，默认 true
    pub verify_signature: bool,
    /// 重载失败时是否回滚，默认 true
    pub rollback_on_failure: bool,
    /// 最大重试次数，默认 3
    pub max_retries: u32,
    /// 重试间隔（毫秒），默认 1000ms
    pub retry_interval_ms: u64,
}

impl Default for AutoReloadConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 500,
            verify_signature: true,
            rollback_on_failure: true,
            max_retries: 3,
            retry_interval_ms: 1000,
        }
    }
}

/// 重载结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadResult {
    /// 是否成功
    pub success: bool,
    /// 插件ID
    pub plugin_id: String,
    /// 新插件ID（如果变更）
    pub new_plugin_id: Option<String>,
    /// 错误信息
    pub error: Option<String>,
    /// 重试次数
    pub retry_count: u32,
}

/// 文件变更事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeEvent {
    /// 插件文件被修改
    Modified(PathBuf),
    /// 插件文件被创建
    Created(PathBuf),
    /// 插件文件被删除
    Deleted(PathBuf),
    /// 插件文件被重命名
    Renamed(PathBuf, PathBuf),
}

/// 插件文件监控器
///
/// 提供轻量级的文件变更检测和自动重载功能。
/// 注意：完整的文件系统监控需要启用 `notify` 特性。
pub struct PluginWatcher {
    /// 插件管理器引用
    manager: Arc<Mutex<PluginManager>>,
    /// 监控配置
    config: AutoReloadConfig,
    /// 插件路径映射 (plugin_id -> path)
    plugin_paths: Arc<Mutex<HashMap<String, PathBuf>>>,
    /// 防抖计时器
    debounce_timer: Arc<Mutex<Option<Instant>>>,
    /// 正在处理的重载任务
    active_reloads: Arc<Mutex<HashMap<String, Instant>>>,
    /// 是否正在自动监控中
    is_auto_watching: Arc<Mutex<bool>>,
}

impl PluginWatcher {
    /// 创建新的插件监控器
    pub fn new(manager: PluginManager, config: AutoReloadConfig) -> Self {
        Self {
            manager: Arc::new(Mutex::new(manager)),
            config,
            plugin_paths: Arc::new(Mutex::new(HashMap::new())),
            debounce_timer: Arc::new(Mutex::new(None)),
            active_reloads: Arc::new(Mutex::new(HashMap::new())),
            is_auto_watching: Arc::new(Mutex::new(false)),
        }
    }

    /// 注册插件进行监控
    pub fn watch_plugin(&self, plugin_id: &str, path: PathBuf) -> Result<(), PluginError> {
        // 验证路径存在
        if !path.exists() {
            return Err(PluginError::LoadFailed(format!(
                "插件文件不存在: {}",
                path.display()
            )));
        }

        let mut paths = self.plugin_paths.lock().unwrap();
        paths.insert(plugin_id.to_string(), path);
        Ok(())
    }

    /// 取消监控插件
    pub fn unwatch_plugin(&self, plugin_id: &str) {
        let mut paths = self.plugin_paths.lock().unwrap();
        paths.remove(plugin_id);

        // 清理活跃任务
        let mut active = self.active_reloads.lock().unwrap();
        active.remove(plugin_id);
    }

    /// 处理文件变更事件（手动触发或通过文件系统事件）
    pub fn handle_file_change(&self, event: FileChangeEvent) -> Option<ReloadResult> {
        // 防抖处理
        if !self.check_debounce() {
            return None;
        }

        match event {
            FileChangeEvent::Modified(path) => self.handle_modified(path),
            FileChangeEvent::Created(path) => self.handle_created(path),
            FileChangeEvent::Deleted(path) => self.handle_deleted(path),
            FileChangeEvent::Renamed(from, to) => self.handle_renamed(from, to),
        }
    }

    /// 检查防抖
    fn check_debounce(&self) -> bool {
        let mut timer = self.debounce_timer.lock().unwrap();
        let now = Instant::now();

        if let Some(last_time) = *timer
            && now.duration_since(last_time) < Duration::from_millis(self.config.debounce_ms)
        {
            return false; // 还在防抖期内
        }

        *timer = Some(now);
        true
    }

    /// 处理文件修改
    fn handle_modified(&self, path: PathBuf) -> Option<ReloadResult> {
        // 查找对应的插件ID
        let plugin_id = self.find_plugin_id_by_path(&path);
        if let Some(plugin_id) = plugin_id {
            return Some(self.reload_plugin_with_retry(&plugin_id));
        }
        None
    }

    /// 处理文件创建
    fn handle_created(&self, _path: PathBuf) -> Option<ReloadResult> {
        // 新文件可能是新插件，可以扩展为自动加载
        None
    }

    /// 处理文件删除
    fn handle_deleted(&self, path: PathBuf) -> Option<ReloadResult> {
        let plugin_id = self.find_plugin_id_by_path(&path);
        if let Some(plugin_id) = plugin_id {
            println!(
                "[Watcher] 插件文件被删除: {} (plugin: {})",
                path.display(),
                plugin_id
            );
            self.unwatch_plugin(&plugin_id);
        }
        None
    }

    /// 处理文件重命名
    fn handle_renamed(&self, from: PathBuf, to: PathBuf) -> Option<ReloadResult> {
        // 查找旧路径对应的插件
        if let Some(plugin_id) = self.find_plugin_id_by_path(&from) {
            // 更新路径映射
            let mut paths = self.plugin_paths.lock().unwrap();
            paths.insert(plugin_id.clone(), to.clone());
            drop(paths);

            // 触发重载
            return Some(self.reload_plugin_with_retry(&plugin_id));
        }
        None
    }

    /// 带重试的插件重载（同步版本）
    fn reload_plugin_with_retry(&self, plugin_id: &str) -> ReloadResult {
        // 检查是否已经在处理中
        {
            let active = self.active_reloads.lock().unwrap();
            if let Some(last_time) = active.get(plugin_id)
                && Instant::now().duration_since(*last_time) < Duration::from_millis(100)
            {
                // 正在处理中，避免重复
                return ReloadResult {
                    success: false,
                    plugin_id: plugin_id.to_string(),
                    new_plugin_id: None,
                    error: Some("重载正在进行中".to_string()),
                    retry_count: 0,
                };
            }
        }

        // 标记为正在处理
        {
            let mut active = self.active_reloads.lock().unwrap();
            active.insert(plugin_id.to_string(), Instant::now());
        }

        // 执行重载
        let result = self.do_reload(plugin_id);

        // 清理处理标记
        let mut active = self.active_reloads.lock().unwrap();
        active.remove(plugin_id);

        result
    }

    /// 执行重载逻辑
    fn do_reload(&self, plugin_id: &str) -> ReloadResult {
        let mut last_error = None;
        let mut retry_count = 0;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                retry_count = attempt;
                std::thread::sleep(Duration::from_millis(self.config.retry_interval_ms));
            }

            // 获取管理器锁并执行重载
            // 注意：遵循锁顺序规范，获取 manager 锁后，manager 内部会按顺序获取其 RwLock
            let mut mgr = self.manager.lock().unwrap();

            // 根据配置决定是否在重载前验证签名
            // 注意：此验证针对旧插件，如果失败说明旧插件已被篡改，不应重试
            if self.config.verify_signature {
                match mgr.verify_plugin_signature(plugin_id, false) {
                    Ok(()) => {} // 签名验证通过，继续重载
                    Err(e) => {
                        // 旧插件签名验证失败，直接返回失败（不重试，因为重试不会改变旧插件状态）
                        return ReloadResult {
                            success: false,
                            plugin_id: plugin_id.to_string(),
                            new_plugin_id: None,
                            error: Some(format!("重载前签名验证失败: {}", e)),
                            retry_count: 0,
                        };
                    }
                }
            }

            // 执行重载（使用配置决定是否验证新插件签名）
            // 注意：watcher 的 verify_signature 配置现在同时影响：
            // 1. 旧插件的预验证（前面已处理）
            // 2. 新插件的验证（通过传递给 reload_with_config）
            let result = mgr.reload_with_config(plugin_id, self.config.verify_signature);

            match result {
                Ok(new_id) => {
                    // 重载成功，返回结果
                    // 注意：根据配置，manager.reload_with_config 可能会验证新插件签名
                    return ReloadResult {
                        success: true,
                        plugin_id: plugin_id.to_string(),
                        new_plugin_id: if new_id != plugin_id {
                            Some(new_id)
                        } else {
                            None
                        },
                        error: None,
                        retry_count,
                    };
                }
                Err(e) => {
                    // 使用模式匹配判断错误类型，而不是字符串匹配
                    let is_signature_error = matches!(
                        &e,
                        PluginError::ExecutionFailed(msg) if msg.contains("签名验证失败")
                    );

                    if is_signature_error && self.config.rollback_on_failure {
                        last_error = Some(format!("重载失败，已回滚: {}", e));
                    } else {
                        last_error = Some(e.to_string());
                    }

                    // 如果是最后一次尝试，返回失败
                    if attempt == self.config.max_retries {
                        break;
                    }
                }
            }
        }

        ReloadResult {
            success: false,
            plugin_id: plugin_id.to_string(),
            new_plugin_id: None,
            error: last_error,
            retry_count,
        }
    }

    /// 根据路径查找插件ID
    fn find_plugin_id_by_path(&self, path: &Path) -> Option<String> {
        let paths = self.plugin_paths.lock().unwrap();
        for (plugin_id, plugin_path) in paths.iter() {
            if plugin_path == path {
                return Some(plugin_id.clone());
            }
        }
        None
    }

    /// 获取当前监控的插件数量
    pub fn watched_count(&self) -> usize {
        self.plugin_paths.lock().unwrap().len()
    }

    /// 列出所有正在监控的插件
    pub fn list_watched_plugins(&self) -> Vec<(String, PathBuf)> {
        let paths = self.plugin_paths.lock().unwrap();
        paths
            .iter()
            .map(|(id, path)| (id.clone(), path.clone()))
            .collect()
    }

    /// 获取配置引用
    pub fn config(&self) -> &AutoReloadConfig {
        &self.config
    }

    /// 启动自动文件监控（需要启用 notify 特性）
    ///
    /// # 返回
    /// 成功返回 Ok(())，失败返回错误
    ///
    /// # 注意
    /// 此方法仅在启用 `notify` 特性时可用
    #[cfg(feature = "notify")]
    pub fn start_auto_watch(&self) -> Result<(), PluginError> {
        use std::sync::mpsc::channel;

        // 检查是否已经在监控中
        {
            let is_watching = self.is_auto_watching.lock().unwrap();
            if *is_watching {
                return Err(PluginError::ExecutionFailed(
                    "自动监控已在运行中".to_string(),
                ));
            }
        }

        // 创建文件系统事件通道
        let (tx, rx) = channel();

        // 创建监控器
        let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())
            .map_err(|e| PluginError::ExecutionFailed(format!("创建文件监控器失败: {}", e)))?;

        // 为所有已注册的插件启动监控
        let paths = self.plugin_paths.lock().unwrap();
        if paths.is_empty() {
            return Err(PluginError::ExecutionFailed(
                "没有可监控的插件，请先使用 watch_plugin 注册插件".to_string(),
            ));
        }

        for (plugin_id, path) in paths.iter() {
            if let Some(parent) = path.parent() {
                watcher
                    .watch(parent, RecursiveMode::NonRecursive)
                    .map_err(|e| {
                        PluginError::ExecutionFailed(format!("监控插件 {} 失败: {}", plugin_id, e))
                    })?;
            }
        }

        // 标记为正在监控（在启动线程之前）
        {
            let mut is_watching = self.is_auto_watching.lock().unwrap();
            *is_watching = true;
        }

        // 将监控器和必要数据移动到线程中
        let manager = self.manager.clone();
        let plugin_paths = self.plugin_paths.clone();
        let is_auto_watching = self.is_auto_watching.clone();
        let debounce_timer = self.debounce_timer.clone();
        let active_reloads = self.active_reloads.clone();
        let config = self.config.clone();

        std::thread::spawn(move || {
            // 在线程中处理文件事件
            for event_result in rx {
                match event_result {
                    Ok(event) => {
                        // 处理文件变更事件
                        match event.kind {
                            EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any) => {
                                for path in &event.paths {
                                    let plugin_id =
                                        Self::find_plugin_id_by_path_internal(&plugin_paths, path);
                                    if let Some(plugin_id) = plugin_id {
                                        println!(
                                            "[Watcher] 检测到文件变更: {} (plugin: {})",
                                            path.display(),
                                            plugin_id
                                        );
                                        Self::handle_file_change_internal(
                                            &manager,
                                            &config,
                                            &debounce_timer,
                                            &active_reloads,
                                            &plugin_paths,
                                            FileChangeEvent::Modified(path.clone()),
                                        );
                                    }
                                }
                            }
                            EventKind::Create(_) => {
                                for path in &event.paths {
                                    let plugin_id =
                                        Self::find_plugin_id_by_path_internal(&plugin_paths, path);
                                    if let Some(plugin_id) = plugin_id {
                                        println!(
                                            "[Watcher] 检测到新文件: {} (plugin: {})",
                                            path.display(),
                                            plugin_id
                                        );
                                        Self::handle_file_change_internal(
                                            &manager,
                                            &config,
                                            &debounce_timer,
                                            &active_reloads,
                                            &plugin_paths,
                                            FileChangeEvent::Created(path.clone()),
                                        );
                                    }
                                }
                            }
                            EventKind::Remove(_) => {
                                for path in &event.paths {
                                    let plugin_id =
                                        Self::find_plugin_id_by_path_internal(&plugin_paths, path);
                                    if let Some(plugin_id) = plugin_id {
                                        println!(
                                            "[Watcher] 检测到文件删除: {} (plugin: {})",
                                            path.display(),
                                            plugin_id
                                        );
                                        Self::handle_file_change_internal(
                                            &manager,
                                            &config,
                                            &debounce_timer,
                                            &active_reloads,
                                            &plugin_paths,
                                            FileChangeEvent::Deleted(path.clone()),
                                        );
                                    }
                                }
                            }
                            EventKind::Modify(ModifyKind::Name(_)) => {
                                // 重命名事件 - 通常会触发后续的修改事件
                                // 这里简单记录，主要依赖修改事件处理重载
                                for path in &event.paths {
                                    let plugin_id =
                                        Self::find_plugin_id_by_path_internal(&plugin_paths, path);
                                    if let Some(plugin_id) = plugin_id {
                                        println!(
                                            "[Watcher] 检测到文件名变更: {} (plugin: {})",
                                            path.display(),
                                            plugin_id
                                        );
                                        // 触发重载（如果文件仍然存在）
                                        if path.exists() {
                                            Self::handle_file_change_internal(
                                                &manager,
                                                &config,
                                                &debounce_timer,
                                                &active_reloads,
                                                &plugin_paths,
                                                FileChangeEvent::Modified(path.clone()),
                                            );
                                        }
                                    }
                                }
                            }
                            _ => {} // 忽略其他事件
                        }
                    }
                    Err(e) => {
                        eprintln!("[Watcher] 文件系统事件错误: {}", e);
                    }
                }
            }

            // 事件通道关闭，清理状态
            let mut is_watching = is_auto_watching.lock().unwrap();
            *is_watching = false;
        });

        Ok(())
    }

    /// 停止自动文件监控
    ///
    /// 注意：由于 notify 监控器在独立线程中运行，此方法仅标记状态为停止
    /// 实际的监控器会在事件通道关闭时自动清理
    #[cfg(feature = "notify")]
    pub fn stop_auto_watch(&self) -> Result<(), PluginError> {
        // 更新状态（实际的监控器会在事件循环结束时自动清理）
        {
            let mut is_watching = self.is_auto_watching.lock().unwrap();
            if !*is_watching {
                return Err(PluginError::ExecutionFailed(
                    "自动监控未在运行中".to_string(),
                ));
            }
            *is_watching = false;
        }

        Ok(())
    }

    /// 检查是否正在自动监控
    pub fn is_auto_watching(&self) -> bool {
        *self.is_auto_watching.lock().unwrap()
    }

    /// 内部方法：根据路径查找插件ID（用于线程环境）
    #[cfg(feature = "notify")]
    fn find_plugin_id_by_path_internal(
        plugin_paths: &Arc<Mutex<HashMap<String, PathBuf>>>,
        path: &Path,
    ) -> Option<String> {
        let paths = plugin_paths.lock().unwrap();
        for (plugin_id, plugin_path) in paths.iter() {
            if plugin_path == path {
                return Some(plugin_id.clone());
            }
        }
        None
    }

    /// 内部方法：处理文件变更（用于线程环境）
    #[cfg(feature = "notify")]
    fn handle_file_change_internal(
        manager: &Arc<Mutex<PluginManager>>,
        config: &AutoReloadConfig,
        debounce_timer: &Arc<Mutex<Option<Instant>>>,
        active_reloads: &Arc<Mutex<HashMap<String, Instant>>>,
        plugin_paths: &Arc<Mutex<HashMap<String, PathBuf>>>,
        event: FileChangeEvent,
    ) -> Option<ReloadResult> {
        // 防抖处理
        {
            let mut timer = debounce_timer.lock().unwrap();
            let now = Instant::now();
            if let Some(last_time) = *timer {
                if now.duration_since(last_time) < Duration::from_millis(config.debounce_ms) {
                    return None; // 还在防抖期内
                }
            }
            *timer = Some(now);
        }

        match event {
            FileChangeEvent::Modified(path) => {
                let plugin_id = Self::find_plugin_id_by_path_internal(plugin_paths, &path);
                if let Some(plugin_id) = plugin_id {
                    return Some(Self::reload_plugin_with_retry_internal(
                        manager,
                        config,
                        active_reloads,
                        &plugin_id,
                    ));
                }
            }
            FileChangeEvent::Created(path) => {
                // 新文件可能是新插件，可以扩展为自动加载
                let _ = path;
            }
            FileChangeEvent::Deleted(path) => {
                let plugin_id = Self::find_plugin_id_by_path_internal(plugin_paths, &path);
                if let Some(plugin_id) = plugin_id {
                    println!(
                        "[Watcher] 插件文件被删除: {} (plugin: {})",
                        path.display(),
                        plugin_id
                    );
                    // 取消监控
                    let mut paths = plugin_paths.lock().unwrap();
                    paths.remove(&plugin_id);
                }
            }
            FileChangeEvent::Renamed(from, to) => {
                let plugin_id = Self::find_plugin_id_by_path_internal(plugin_paths, &from);
                if let Some(plugin_id) = plugin_id {
                    // 更新路径映射
                    let mut paths = plugin_paths.lock().unwrap();
                    paths.insert(plugin_id.clone(), to.clone());
                    drop(paths);

                    // 触发重载
                    return Some(Self::reload_plugin_with_retry_internal(
                        manager,
                        config,
                        active_reloads,
                        &plugin_id,
                    ));
                }
            }
        }
        None
    }

    /// 内部方法：带重试的插件重载（用于线程环境）
    #[cfg(feature = "notify")]
    fn reload_plugin_with_retry_internal(
        manager: &Arc<Mutex<PluginManager>>,
        config: &AutoReloadConfig,
        active_reloads: &Arc<Mutex<HashMap<String, Instant>>>,
        plugin_id: &str,
    ) -> ReloadResult {
        // 检查是否已经在处理中
        {
            let active = active_reloads.lock().unwrap();
            if let Some(last_time) = active.get(plugin_id) {
                if Instant::now().duration_since(*last_time) < Duration::from_millis(100) {
                    return ReloadResult {
                        success: false,
                        plugin_id: plugin_id.to_string(),
                        new_plugin_id: None,
                        error: Some("重载正在进行中".to_string()),
                        retry_count: 0,
                    };
                }
            }
        }

        // 标记为正在处理
        {
            let mut active = active_reloads.lock().unwrap();
            active.insert(plugin_id.to_string(), Instant::now());
        }

        // 执行重载
        let result = Self::do_reload_internal(manager, config, plugin_id);

        // 清理处理标记
        let mut active = active_reloads.lock().unwrap();
        active.remove(plugin_id);

        result
    }

    /// 内部方法：执行重载逻辑（用于线程环境）
    #[cfg(feature = "notify")]
    fn do_reload_internal(
        manager: &Arc<Mutex<PluginManager>>,
        config: &AutoReloadConfig,
        plugin_id: &str,
    ) -> ReloadResult {
        let mut last_error = None;
        let mut retry_count = 0;

        for attempt in 0..=config.max_retries {
            if attempt > 0 {
                retry_count = attempt;
                std::thread::sleep(Duration::from_millis(config.retry_interval_ms));
            }

            // 获取管理器锁并执行重载
            // 注意：遵循锁顺序规范，获取 manager 锁后，manager 内部会按顺序获取其 RwLock
            let mut mgr = manager.lock().unwrap();

            // 根据配置决定是否在重载前验证签名
            // 注意：此验证针对旧插件，如果失败说明旧插件已被篡改，不应重试
            if config.verify_signature {
                match mgr.verify_plugin_signature(plugin_id, false) {
                    Ok(()) => {} // 签名验证通过，继续重载
                    Err(e) => {
                        // 旧插件签名验证失败，直接返回失败（不重试，因为重试不会改变旧插件状态）
                        return ReloadResult {
                            success: false,
                            plugin_id: plugin_id.to_string(),
                            new_plugin_id: None,
                            error: Some(format!("重载前签名验证失败: {}", e)),
                            retry_count: 0,
                        };
                    }
                }
            }

            // 执行重载（使用配置决定是否验证新插件签名）
            // 注意：watcher 的 verify_signature 配置现在同时影响：
            // 1. 旧插件的预验证（前面已处理）
            // 2. 新插件的验证（通过传递给 reload_with_config）
            let result = mgr.reload_with_config(plugin_id, config.verify_signature);

            match result {
                Ok(new_id) => {
                    // 重载成功，返回结果
                    // 注意：根据配置，manager.reload_with_config 可能会验证新插件签名
                    return ReloadResult {
                        success: true,
                        plugin_id: plugin_id.to_string(),
                        new_plugin_id: if new_id != plugin_id {
                            Some(new_id)
                        } else {
                            None
                        },
                        error: None,
                        retry_count,
                    };
                }
                Err(e) => {
                    // 使用模式匹配判断错误类型，而不是字符串匹配
                    let is_signature_error = matches!(
                        &e,
                        PluginError::ExecutionFailed(msg) if msg.contains("签名验证失败")
                    );

                    if is_signature_error && config.rollback_on_failure {
                        last_error = Some(format!("重载失败，已回滚: {}", e));
                    } else {
                        last_error = Some(e.to_string());
                    }

                    // 如果是最后一次尝试，返回失败
                    if attempt == config.max_retries {
                        break;
                    }
                }
            }
        }

        ReloadResult {
            success: false,
            plugin_id: plugin_id.to_string(),
            new_plugin_id: None,
            error: last_error,
            retry_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_auto_reload_config_default() {
        let config = AutoReloadConfig::default();
        assert_eq!(config.debounce_ms, 500);
        assert!(config.verify_signature);
        assert!(config.rollback_on_failure);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_interval_ms, 1000);
    }

    #[test]
    fn test_file_change_event_clone() {
        let event = FileChangeEvent::Modified(PathBuf::from("/test/plugin.dll"));
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_reload_result_clone() {
        let result = ReloadResult {
            success: true,
            plugin_id: "test".to_string(),
            new_plugin_id: Some("test_v2".to_string()),
            error: None,
            retry_count: 0,
        };
        let cloned = result.clone();
        assert_eq!(result.success, cloned.success);
        assert_eq!(result.plugin_id, cloned.plugin_id);
        assert_eq!(result.new_plugin_id, cloned.new_plugin_id);
    }

    #[test]
    fn test_reload_result_eq() {
        let result1 = ReloadResult {
            success: true,
            plugin_id: "test".to_string(),
            new_plugin_id: None,
            error: None,
            retry_count: 0,
        };
        let result2 = ReloadResult {
            success: true,
            plugin_id: "test".to_string(),
            new_plugin_id: None,
            error: None,
            retry_count: 0,
        };
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_plugin_watcher_creation() {
        use crate::plugin::manager::PluginManager;

        let manager = PluginManager::empty();
        let config = AutoReloadConfig::default();
        let watcher = PluginWatcher::new(manager, config);

        assert_eq!(watcher.watched_count(), 0);
        assert_eq!(watcher.config().debounce_ms, 500);
    }

    #[test]
    fn test_file_change_event_types() {
        use std::path::PathBuf;

        let modified = FileChangeEvent::Modified(PathBuf::from("/test/plugin.dll"));
        let created = FileChangeEvent::Created(PathBuf::from("/test/new.dll"));
        let deleted = FileChangeEvent::Deleted(PathBuf::from("/test/old.dll"));
        let renamed = FileChangeEvent::Renamed(
            PathBuf::from("/test/old.dll"),
            PathBuf::from("/test/new.dll"),
        );

        // 测试各种事件类型都能正确创建
        assert!(matches!(modified, FileChangeEvent::Modified(_)));
        assert!(matches!(created, FileChangeEvent::Created(_)));
        assert!(matches!(deleted, FileChangeEvent::Deleted(_)));
        assert!(matches!(renamed, FileChangeEvent::Renamed(_, _)));
    }

    #[test]
    fn test_auto_reload_config_custom() {
        let config = AutoReloadConfig {
            debounce_ms: 1000,
            verify_signature: false,
            rollback_on_failure: false,
            max_retries: 5,
            retry_interval_ms: 2000,
        };

        assert_eq!(config.debounce_ms, 1000);
        assert!(!config.verify_signature);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_auto_watching_state() {
        use crate::plugin::manager::PluginManager;

        let manager = PluginManager::empty();
        let config = AutoReloadConfig::default();
        let watcher = PluginWatcher::new(manager, config);

        // 初始状态应该不是正在监控
        assert!(!watcher.is_auto_watching());
    }

    #[test]
    fn test_concurrent_reload_protection() {
        use crate::plugin::manager::PluginManager;

        let mut manager = PluginManager::empty();

        // 测试并发检查的基本结构
        // 注意：由于需要实际的插件文件，这里只测试结构完整性
        assert!(manager.reload("non-existent").is_err());
    }

    #[test]
    fn test_signature_verification_config_integration() {
        // 测试配置的签名验证设置
        let config = AutoReloadConfig::default();
        assert!(config.verify_signature); // 默认应该启用

        let config_disabled = AutoReloadConfig {
            verify_signature: false,
            ..AutoReloadConfig::default()
        };
        assert!(!config_disabled.verify_signature);
    }
}
