//! 钩子系统
//!
//! 管理和执行插件钩子，支持优先级和错误处理

use crate::plugin::types::{HookContext, HookPriority, HookResult, HookType, Plugin, PluginError};
use std::collections::HashMap;
use std::sync::Arc;

/// 钩子注册项
struct HookRegistration {
    plugin: Arc<dyn Plugin>,
    priority: HookPriority,
    enabled: bool,
}

/// 钩子调度器
pub struct HookDispatcher {
    /// 按钩子类型和优先级存储的插件
    hooks: HashMap<HookType, Vec<HookRegistration>>,
}

impl HookDispatcher {
    /// 创建钩子调度器
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// 注册插件钩子
    pub fn register(
        &mut self,
        hook_type: HookType,
        plugin: Arc<dyn Plugin>,
        priority: HookPriority,
    ) -> Result<(), PluginError> {
        let registration = HookRegistration {
            plugin,
            priority,
            enabled: true,
        };

        self.hooks
            .entry(hook_type.clone())
            .or_default()
            .push(registration);

        // 按优先级排序
        if let Some(hooks) = self.hooks.get_mut(&hook_type) {
            hooks.sort_by(|a, b| a.priority.cmp(&b.priority));
        }

        Ok(())
    }

    /// 注销插件的所有钩子
    pub fn unregister(&mut self, plugin_id: &str) {
        for hooks in self.hooks.values_mut() {
            hooks.retain(|reg| {
                let metadata = reg.plugin.metadata();
                metadata.id != plugin_id
            });
        }

        // 清理空的钩子类型
        self.hooks.retain(|_, hooks| !hooks.is_empty());
    }

    /// 禁用插件的特定钩子
    pub fn disable_hook(
        &mut self,
        hook_type: HookType,
        plugin_id: &str,
    ) -> Result<(), PluginError> {
        if let Some(hooks) = self.hooks.get_mut(&hook_type) {
            for reg in hooks {
                if reg.plugin.metadata().id == plugin_id {
                    reg.enabled = false;
                    return Ok(());
                }
            }
        }
        Err(PluginError::NotFound(plugin_id.to_string()))
    }

    /// 启用插件的特定钩子
    pub fn enable_hook(&mut self, hook_type: HookType, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(hooks) = self.hooks.get_mut(&hook_type) {
            for reg in hooks {
                if reg.plugin.metadata().id == plugin_id {
                    reg.enabled = true;
                    return Ok(());
                }
            }
        }
        Err(PluginError::NotFound(plugin_id.to_string()))
    }

    /// 执行单个钩子类型的所有插件
    pub fn execute(
        &self,
        hook_type: HookType,
        context: &HookContext,
    ) -> Result<Vec<HookResult>, PluginError> {
        let mut results = Vec::new();
        let mut continue_execution = true;

        // 获取该类型的所有钩子
        if let Some(hooks) = self.hooks.get(&hook_type) {
            for reg in hooks {
                if !reg.enabled {
                    continue;
                }

                // 检查是否应该继续执行
                if !continue_execution {
                    break;
                }

                // 执行钩子
                match reg.plugin.execute_hook(hook_type.clone(), context) {
                    Ok(result) => {
                        // 更新继续执行状态
                        if !result.continue_execution {
                            continue_execution = false;
                        }
                        results.push(result);
                    }
                    Err(e) => {
                        // 钩子执行失败，记录但继续执行其他钩子
                        // 除非是 Critical 优先级的钩子
                        if reg.priority <= HookPriority::CRITICAL {
                            return Err(e);
                        }
                        // 记录错误但继续
                        results.push(HookResult {
                            continue_execution: true,
                            message: Some(format!("Hook failed: {}", e)),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// 执行钩子链（支持上下文修改）
    ///
    /// 这个方法会累积钩子的修改，并传递给下一个钩子
    pub fn execute_with_context<'a>(
        &self,
        hook_type: HookType,
        mut context: HookContext<'a>,
    ) -> Result<(HookContext<'a>, Vec<HookResult>), PluginError> {
        let mut results = Vec::new();

        if let Some(hooks) = self.hooks.get(&hook_type) {
            for reg in hooks {
                if !reg.enabled {
                    continue;
                }

                if !context.continue_execution {
                    break;
                }

                // 执行钩子
                let result = reg.plugin.execute_hook(hook_type.clone(), &context)?;

                // 累积环境变量修改
                for (key, value) in &result.modified_env {
                    context.env.insert(key.clone(), value.clone());
                }

                // 累积插件数据
                for (key, value) in &result.plugin_data {
                    context.plugin_data.insert(key.clone(), value.clone());
                }

                // 更新继续执行状态
                if !result.continue_execution {
                    context.continue_execution = false;
                }

                results.push(result);
            }
        }

        Ok((context, results))
    }

    /// 获取指定钩子类型的所有注册插件
    pub fn get_hooks(&self, hook_type: HookType) -> Vec<(String, HookPriority, bool)> {
        self.hooks
            .get(&hook_type)
            .map(|hooks| {
                hooks
                    .iter()
                    .map(|reg| {
                        (
                            reg.plugin.metadata().id.clone(),
                            reg.priority.clone(),
                            reg.enabled,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 检查是否有指定类型的钩子
    pub fn has_hooks(&self, hook_type: HookType) -> bool {
        self.hooks
            .get(&hook_type)
            .map(|hooks| hooks.iter().any(|reg| reg.enabled))
            .unwrap_or(false)
    }

    /// 获取所有钩子类型
    pub fn get_all_hook_types(&self) -> Vec<HookType> {
        self.hooks.keys().cloned().collect()
    }

    /// 清空所有钩子
    pub fn clear(&mut self) {
        self.hooks.clear();
    }

    /// 获取钩子统计信息
    pub fn get_stats(&self) -> HookStats {
        let mut stats = HookStats::default();

        for (hook_type, hooks) in &self.hooks {
            let enabled_count = hooks.iter().filter(|r| r.enabled).count();
            stats.total_hooks += hooks.len();
            stats.enabled_hooks += enabled_count;

            match hook_type {
                HookType::PreCommand => stats.pre_command_hooks = enabled_count,
                HookType::PostCommand => stats.post_command_hooks = enabled_count,
                HookType::Error => stats.error_hooks = enabled_count,
                _ => {}
            }
        }

        stats
    }
}

impl Default for HookDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// 钩子统计信息
#[derive(Debug, Clone, Default)]
pub struct HookStats {
    pub total_hooks: usize,
    pub enabled_hooks: usize,
    pub pre_command_hooks: usize,
    pub post_command_hooks: usize,
    pub error_hooks: usize,
}

/// 钩子执行器（带超时支持）
pub struct HookExecutor {
    dispatcher: HookDispatcher,
    default_timeout: std::time::Duration,
}

impl HookExecutor {
    pub fn new(default_timeout: std::time::Duration) -> Self {
        Self {
            dispatcher: HookDispatcher::new(),
            default_timeout,
        }
    }

    /// 执行钩子（带超时）
    pub fn execute_with_timeout(
        &self,
        hook_type: HookType,
        context: &HookContext,
        timeout: Option<std::time::Duration>,
    ) -> Result<Vec<HookResult>, PluginError> {
        let _timeout = timeout.unwrap_or(self.default_timeout);

        // 简单实现：这里不使用真正的超时机制（需要 async）
        // 在实际实现中，应该使用 tokio::time::timeout
        // 为了保持同步 API，我们只记录超时配置

        self.dispatcher.execute(hook_type, context)
    }

    /// 获取内部调度器（用于配置）
    pub fn dispatcher_mut(&mut self) -> &mut HookDispatcher {
        &mut self.dispatcher
    }

    /// 获取内部调度器（只读）
    pub fn dispatcher(&self) -> &HookDispatcher {
        &self.dispatcher
    }
}

/// 钩子错误处理器
pub struct HookErrorHandler {
    dispatcher: HookDispatcher,
}

impl HookErrorHandler {
    pub fn new() -> Self {
        Self {
            dispatcher: HookDispatcher::new(),
        }
    }

    /// 处理错误（调用 Error 钩子）
    pub fn handle_error(
        &self,
        error: &PluginError,
        context: &HookContext,
    ) -> Result<Vec<HookResult>, PluginError> {
        // 创建错误上下文
        let mut error_context = context.clone();
        error_context.error = Some(error.to_string());

        // 执行错误钩子
        self.dispatcher.execute(HookType::Error, &error_context)
    }

    /// 注册错误处理插件
    pub fn register_error_handler(
        &mut self,
        plugin: Arc<dyn Plugin>,
        priority: HookPriority,
    ) -> Result<(), PluginError> {
        self.dispatcher.register(HookType::Error, plugin, priority)
    }

    /// 获取错误处理器统计
    pub fn get_stats(&self) -> HookStats {
        self.dispatcher.get_stats()
    }
}

impl Default for HookErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// 钩子链构建器（用于流式配置）
pub struct HookChainBuilder {
    hooks: Vec<(HookType, Arc<dyn Plugin>, HookPriority)>,
}

impl HookChainBuilder {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// 添加钩子
    pub fn add(
        mut self,
        hook_type: HookType,
        plugin: Arc<dyn Plugin>,
        priority: HookPriority,
    ) -> Self {
        self.hooks.push((hook_type, plugin, priority));
        self
    }

    /// 构建钩子调度器
    pub fn build(self) -> Result<HookDispatcher, PluginError> {
        let mut dispatcher = HookDispatcher::new();

        for (hook_type, plugin, priority) in self.hooks {
            dispatcher.register(hook_type, plugin, priority)?;
        }

        Ok(dispatcher)
    }
}

impl Default for HookChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::types::PluginMetadata;
    use std::sync::Mutex;

    // 测试插件
    struct TestPlugin {
        metadata: PluginMetadata,
        exec_count: Mutex<usize>,
    }

    impl TestPlugin {
        fn new(id: &str, hooks: Vec<HookType>) -> Self {
            Self {
                metadata: PluginMetadata {
                    id: id.to_string(),
                    name: id.to_string(),
                    version: "1.0.0".to_string(),
                    description: None,
                    author: None,
                    plugin_type: crate::plugin::types::PluginType::DynamicLibrary,
                    hooks: hooks.clone(),
                    extensions: vec![],
                    config_schema: None,
                    enabled: true,
                    dependencies: vec![],
                    platforms: vec![crate::plugin::types::Platform::current()],
                    envcli_version: None,
                    signature: None,
                },
                exec_count: Mutex::new(0),
            }
        }
    }

    impl Plugin for TestPlugin {
        fn metadata(&self) -> PluginMetadata {
            self.metadata.clone()
        }

        fn initialize(
            &mut self,
            _config: &crate::plugin::types::PluginConfig,
        ) -> Result<(), PluginError> {
            Ok(())
        }

        fn execute_hook(
            &self,
            _hook_type: HookType,
            _context: &HookContext,
        ) -> Result<HookResult, PluginError> {
            let mut count = self.exec_count.lock().unwrap();
            *count += 1;

            Ok(HookResult {
                message: Some(format!("Executed {} times", *count)),
                continue_execution: true,
                ..Default::default()
            })
        }

        fn supports_extension(&self, _extension: crate::plugin::types::ExtensionPoint) -> bool {
            false
        }

        fn execute_extension(
            &self,
            _extension: crate::plugin::types::ExtensionPoint,
            _input: &[u8],
        ) -> Result<Vec<u8>, PluginError> {
            Err(PluginError::Unsupported("No extensions".to_string()))
        }

        fn shutdown(&mut self) -> Result<(), PluginError> {
            Ok(())
        }
    }

    #[test]
    fn test_hook_dispatcher_register() {
        let mut dispatcher = HookDispatcher::new();
        let plugin = Arc::new(TestPlugin::new("test1", vec![HookType::PreCommand]));

        dispatcher
            .register(HookType::PreCommand, plugin, HookPriority::NORMAL)
            .unwrap();

        assert!(dispatcher.has_hooks(HookType::PreCommand));
        assert_eq!(dispatcher.get_hooks(HookType::PreCommand).len(), 1);
    }

    #[test]
    fn test_hook_priority_order() {
        let mut dispatcher = HookDispatcher::new();

        let plugin_low = Arc::new(TestPlugin::new("low", vec![HookType::PreCommand]));
        let plugin_high = Arc::new(TestPlugin::new("high", vec![HookType::PreCommand]));

        dispatcher
            .register(HookType::PreCommand, plugin_low, HookPriority::LOW)
            .unwrap();
        dispatcher
            .register(HookType::PreCommand, plugin_high, HookPriority::HIGH)
            .unwrap();

        let hooks = dispatcher.get_hooks(HookType::PreCommand);
        // 应该按优先级排序：HIGH (50) 在前，LOW (150) 在后
        assert_eq!(hooks[0].0, "high");
        assert_eq!(hooks[1].0, "low");
    }

    #[test]
    fn test_hook_execution() {
        let mut dispatcher = HookDispatcher::new();
        let plugin = Arc::new(TestPlugin::new("test", vec![HookType::PreCommand]));

        dispatcher
            .register(HookType::PreCommand, plugin, HookPriority::NORMAL)
            .unwrap();

        let context = HookContext {
            command: "run",
            args: &[],
            env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        };

        let results = dispatcher.execute(HookType::PreCommand, &context).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].message.is_some());
    }

    #[test]
    fn test_hook_unregistration() {
        let mut dispatcher = HookDispatcher::new();
        let plugin = Arc::new(TestPlugin::new("test", vec![HookType::PreCommand]));

        dispatcher
            .register(HookType::PreCommand, plugin, HookPriority::NORMAL)
            .unwrap();

        assert!(dispatcher.has_hooks(HookType::PreCommand));

        dispatcher.unregister("test");

        assert!(!dispatcher.has_hooks(HookType::PreCommand));
    }

    #[test]
    fn test_hook_enable_disable() {
        let mut dispatcher = HookDispatcher::new();
        let plugin = Arc::new(TestPlugin::new("test", vec![HookType::PreCommand]));

        dispatcher
            .register(HookType::PreCommand, plugin, HookPriority::NORMAL)
            .unwrap();

        // 禁用
        dispatcher
            .disable_hook(HookType::PreCommand, "test")
            .unwrap();

        let context = HookContext {
            command: "run",
            args: &[],
            env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        };

        // 禁用后不应该执行
        let results = dispatcher.execute(HookType::PreCommand, &context).unwrap();
        assert_eq!(results.len(), 0);

        // 启用
        dispatcher
            .enable_hook(HookType::PreCommand, "test")
            .unwrap();

        let results = dispatcher.execute(HookType::PreCommand, &context).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_hook_stats() {
        let mut dispatcher = HookDispatcher::new();

        let plugin1 = Arc::new(TestPlugin::new("test1", vec![HookType::PreCommand]));
        let plugin2 = Arc::new(TestPlugin::new("test2", vec![HookType::PostCommand]));

        dispatcher
            .register(HookType::PreCommand, plugin1, HookPriority::NORMAL)
            .unwrap();
        dispatcher
            .register(HookType::PostCommand, plugin2, HookPriority::NORMAL)
            .unwrap();

        let stats = dispatcher.get_stats();
        assert_eq!(stats.total_hooks, 2);
        assert_eq!(stats.enabled_hooks, 2);
        assert_eq!(stats.pre_command_hooks, 1);
        assert_eq!(stats.post_command_hooks, 1);
    }

    #[test]
    fn test_hook_error_handler() {
        let mut handler = HookErrorHandler::new();
        let plugin = Arc::new(TestPlugin::new("error_handler", vec![HookType::Error]));

        handler
            .register_error_handler(plugin, HookPriority::NORMAL)
            .unwrap();

        let context = HookContext {
            command: "run",
            args: &[],
            env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        };

        let error = PluginError::ExecutionFailed("Test error".to_string());
        let results = handler.handle_error(&error, &context).unwrap();

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_hook_chain_builder() {
        let plugin1 = Arc::new(TestPlugin::new("test1", vec![HookType::PreCommand]));
        let plugin2 = Arc::new(TestPlugin::new("test2", vec![HookType::PreCommand]));

        let dispatcher = HookChainBuilder::new()
            .add(HookType::PreCommand, plugin1, HookPriority::HIGH)
            .add(HookType::PreCommand, plugin2, HookPriority::LOW)
            .build()
            .unwrap();

        assert!(dispatcher.has_hooks(HookType::PreCommand));
        assert_eq!(dispatcher.get_hooks(HookType::PreCommand).len(), 2);
    }

    #[test]
    fn test_execute_with_context() {
        let mut dispatcher = HookDispatcher::new();
        let plugin = Arc::new(TestPlugin::new("test", vec![HookType::PreCommand]));

        dispatcher
            .register(HookType::PreCommand, plugin, HookPriority::NORMAL)
            .unwrap();

        let context = HookContext {
            command: "run",
            args: &[],
            env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        };

        let (new_context, results) = dispatcher
            .execute_with_context(HookType::PreCommand, context)
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(new_context.continue_execution);
    }
}
