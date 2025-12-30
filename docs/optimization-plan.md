# EnvCLI 性能优化实施计划

**制定日期**: 2025-12-30
**优先级**: P2 (性能优化阶段)
**预计耗时**: 2-3 天

---

## 🎯 优化目标

### 核心指标
- ✅ 减少文件 I/O 操作 80-90%
- ✅ 减少系统环境读取 70-85%
- ✅ 改善响应时间 50-70%
- ✅ 保持代码简洁性和可维护性

---

## 📋 任务清单

### 阶段 1: 系统环境缓存 (P0 - 高优先级)

#### 1.1 创建缓存结构

**文件**: `src/utils/paths.rs`

```rust
use std::sync::Mutex;
use std::time::{Instant, Duration};
use std::collections::HashMap;

/// 系统环境变量缓存
struct SystemEnvCache {
    env: HashMap<String, String>,
    timestamp: Instant,
}

impl SystemEnvCache {
    fn is_valid(&self) -> bool {
        self.timestamp.elapsed() < Duration::from_secs(60)
    }
}

/// 全局缓存实例
static SYSTEM_ENV_CACHE: OnceLock<Mutex<Option<SystemEnvCache>>> = OnceLock::new();
```

#### 1.2 修改 `get_system_env()` 函数

**位置**: `src/utils/paths.rs:140-195`

```rust
pub fn get_system_env() -> Result<HashMap<String, String>> {
    // 获取或初始化缓存
    let cache_guard = SYSTEM_ENV_CACHE.get_or_init(|| Mutex::new(None));
    let mut cache_opt = cache_guard.lock().unwrap();

    // 检查缓存有效性
    if let Some(cache) = &*cache_opt {
        if cache.is_valid() {
            return Ok(cache.env.clone());
        }
    }

    // 缓存失效，重新读取
    let env = read_system_env_from_source()?;

    // 更新缓存
    *cache_opt = Some(SystemEnvCache {
        env: env.clone(),
        timestamp: Instant::now(),
    });

    Ok(env)
}

/// 实际读取系统环境的内部函数
fn read_system_env_from_source() -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();

    #[cfg(target_os = "windows")]
    {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};

        for (key, value) in std::env::vars() {
            if !value.is_empty() && !key.starts_with('_') {
                env.insert(key, value);
            }
        }

        match RegKey::predef(HKEY_CURRENT_USER).open_subkey("Environment") {
            Ok(reg_key) => {
                for (name, _) in reg_key.enum_values().flatten() {
                    if name.starts_with('_') || name == "_" {
                        continue;
                    }
                    if let Ok(value) = reg_key.get_value::<String, _>(&name) {
                        if !value.is_empty() {
                            env.insert(name, value);
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        for (key, value) in std::env::vars() {
            if !value.is_empty() && !key.starts_with('_') {
                env.insert(key, value);
            }
        }
    }

    Ok(env)
}
```

#### 1.3 添加缓存失效控制

```rust
/// 手动清除缓存（用于测试或强制刷新）
pub fn clear_system_env_cache() {
    if let Some(cache) = SYSTEM_ENV_CACHE.get() {
        let mut guard = cache.lock().unwrap();
        *guard = None;
    }
}

/// 获取缓存统计信息
pub fn get_cache_stats() -> (bool, Duration) {
    if let Some(cache) = SYSTEM_ENV_CACHE.get() {
        if let Ok(guard) = cache.lock() {
            if let Some(c) = &*guard {
                return (true, c.timestamp.elapsed());
            }
        }
    }
    (false, Duration::from_secs(0))
}
```

#### 1.4 添加测试

```rust
#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn test_system_env_cache_works() {
        // 第一次调用 - 应该读取系统环境
        let start1 = Instant::now();
        let env1 = get_system_env().unwrap();
        let time1 = start1.elapsed();

        // 第二次调用 - 应该使用缓存
        let start2 = Instant::now();
        let env2 = get_system_env().unwrap();
        let time2 = start2.elapsed();

        // 内容应该相同
        assert_eq!(env1, env2);

        // 第二次应该明显更快
        println!("First call: {:?}, Second call: {:?}", time1, time2);
        // 缓存命中应该 < 1ms，首次可能 > 10ms
    }

    #[test]
    fn test_cache_expiration() {
        // 这个测试需要修改缓存 TTL 为 1 秒用于测试
        // 验证缓存过期后重新读取
    }

    #[test]
    fn test_cache_concurrency() {
        // 测试多线程并发访问缓存
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(|| {
                    get_system_env().unwrap();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
```

**测试验证**:
- ✅ 缓存命中测试
- ✅ 缓存失效测试
- ✅ 并发安全测试
- ✅ 性能对比测试

---

### 阶段 2: Store 文件缓存 (P0 - 高优先级)

#### 2.1 创建文件缓存结构

**文件**: `src/core/store.rs`

```rust
use std::sync::RwLock;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone)]
struct FileCacheEntry {
    vars: Vec<EnvVar>,
    last_modified: SystemTime,
}

/// 全局文件缓存
static FILE_CACHE: OnceLock<RwLock<HashMap<PathBuf, FileCacheEntry>>> = OnceLock::new();
```

#### 2.2 修改 Store 结构

```rust
#[derive(Clone)]
pub struct Store {
    config: Config,
}

impl Store {
    // ... 现有方法 ...

    /// 获取缓存的变量列表
    fn get_cached_vars(&self, path: &PathBuf) -> Result<Option<Vec<EnvVar>>> {
        if !file_exists(path) {
            return Ok(None);
        }

        let cache = FILE_CACHE.get_or_init(|| RwLock::new(HashMap::new())).read().unwrap();

        if let Some(entry) = cache.get(path) {
            // 检查文件是否被修改
            let current_modified = std::fs::metadata(path)?.modified()?;
            if entry.last_modified == current_modified {
                return Ok(Some(entry.vars.clone()));
            }
        }

        Ok(None)
    }

    /// 更新缓存
    fn update_cache(&self, path: &PathBuf, vars: Vec<EnvVar>) -> Result<()> {
        let current_modified = std::fs::metadata(path)?.modified()?;

        let mut cache = FILE_CACHE.get_or_init(|| RwLock::new(HashMap::new())).write().unwrap();

        cache.insert(
            path.clone(),
            FileCacheEntry {
                vars,
                last_modified: current_modified,
            },
        );

        Ok(())
    }

    /// 清除指定路径的缓存
    pub fn invalidate_cache(&self, path: &PathBuf) {
        if let Ok(mut cache) = FILE_CACHE.get_or_init(|| RwLock::new(HashMap::new())).write() {
            cache.remove(path);
        }
    }

    /// 清除所有缓存
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = FILE_CACHE.get_or_init(|| RwLock::new(HashMap::new())).write() {
            cache.clear();
        }
    }
}
```

#### 2.3 优化 `get_from_source()` 方法

```rust
fn get_from_source(&self, key: &str, source: &EnvSource) -> Result<Option<String>> {
    // 系统层特殊处理
    if *source == EnvSource::System {
        let system_env = get_system_env()?;
        return Ok(system_env.get(key).cloned());
    }

    // 获取文件路径
    let path = paths::get_layer_path(source)?;

    // 文件不存在
    if !file_exists(&path) {
        return Ok(None);
    }

    // 尝试从缓存获取
    if let Some(cached_vars) = self.get_cached_vars(&path)? {
        return Ok(cached_vars.iter()
            .find(|v| v.key == key)
            .map(|v| v.value.clone()));
    }

    // 缓存未命中，读取并解析
    let content = read_file(&path)?;
    let vars = DotenvParser::parse(&content, source)?;

    // 更新缓存
    self.update_cache(&path, vars.clone())?;

    // 查找目标变量
    Ok(vars.iter()
        .find(|v| v.key == key)
        .map(|v| v.value.clone()))
}
```

#### 2.4 优化 `list()` 方法

```rust
pub fn list(&self, source: Option<EnvSource>) -> Result<Vec<EnvVar>> {
    match source {
        Some(s) => self.list_from_source(&s),
        None => self.list_merged(),
    }
}

fn list_from_source(&self, source: &EnvSource) -> Result<Vec<EnvVar>> {
    if *source == EnvSource::System {
        return Ok(get_system_env()?
            .into_iter()
            .map(|(k, v)| EnvVar::new(k, v, EnvSource::System))
            .collect());
    }

    let path = paths::get_layer_path(source)?;

    if !file_exists(&path) {
        return Ok(Vec::new());
    }

    // 使用缓存
    if let Some(cached) = self.get_cached_vars(&path)? {
        return Ok(cached);
    }

    let content = read_file(&path)?;
    let vars = DotenvParser::parse(&content, source)?;
    self.update_cache(&path, vars.clone())?;

    Ok(vars)
}
```

#### 2.5 优化 `merge_environment()` 函数

**文件**: `src/utils/env_merge.rs`

```rust
pub fn merge_environment(
    store: &Store,
    temp_vars: &[(String, String)],
) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();

    // 一次性获取所有层级（已按优先级排序）
    let all_vars = store.list(None)?;

    for var in all_vars {
        env.insert(var.key, var.value);
    }

    // 临时变量覆盖
    for (key, value) in temp_vars {
        env.insert(key.clone(), value.clone());
    }

    Ok(env)
}
```

#### 2.6 添加缓存管理命令

**文件**: `src/cli.rs`

```rust
#[derive(Subcommand)]
pub enum CacheCommands {
    /// 清除文件缓存
    ClearFile,

    /// 清除系统环境缓存
    ClearSystem,

    /// 清除所有缓存
    ClearAll,

    /// 显示缓存统计
    Stats,
}

// 在 main.rs 中添加处理函数
fn handle_cache_commands(command: &CacheCommands, verbose: bool) -> Result<()> {
    match command {
        CacheCommands::ClearFile => {
            let store = Store::new(Config { verbose });
            store.clear_cache();
            if verbose {
                println!("✓ 文件缓存已清除");
            }
        }
        CacheCommands::ClearSystem => {
            paths::clear_system_env_cache();
            if verbose {
                println!("✓ 系统环境缓存已清除");
            }
        }
        CacheCommands::ClearAll => {
            let store = Store::new(Config { verbose });
            store.clear_cache();
            paths::clear_system_env_cache();
            if verbose {
                println!("✓ 所有缓存已清除");
            }
        }
        CacheCommands::Stats => {
            // 显示缓存统计信息
            println!("缓存统计:");
            // ... 实现统计逻辑
        }
    }
    Ok(())
}
```

---

### 阶段 3: 性能测试与验证 (P1 - 中优先级)

#### 3.1 创建性能基准测试

**文件**: `benches/performance.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use envcli::{Store, Config};
use std::time::Instant;

fn bench_get_operations(c: &mut Criterion) {
    let store = Store::new(Config { verbose: false });

    c.bench_function("single_get", |b| {
        b.iter(|| store.get(black_box("PATH")))
    });

    c.bench_function("multiple_get", |b| {
        b.iter(|| {
            for _ in 0..10 {
                store.get(black_box("PATH")).unwrap();
            }
        })
    });
}

fn bench_merge_environment(c: &mut Criterion) {
    let store = Store::new(Config { verbose: false });

    c.bench_function("merge_environment", |b| {
        b.iter(|| {
            envcli::utils::env_merge::EnvMerger::merge_environment(
                &store,
                &[("TEMP_VAR".to_string(), "value".to_string())],
            )
        })
    });
}

criterion_group!(benches, bench_get_operations, bench_merge_environment);
criterion_main!(benches);
```

#### 3.2 性能对比测试脚本

**文件**: `scripts/benchmark.sh`

```bash
#!/bin/bash

echo "=== EnvCLI 性能基准测试 ==="

# 测试 1: 单次查询
echo -n "单次查询: "
time envcli get PATH >/dev/null 2>&1

# 测试 2: 100 次连续查询
echo -n "100次查询: "
time for i in {1..100}; do envcli get PATH >/dev/null 2>&1; done

# 测试 3: 环境合并 (run 命令)
echo -n "环境合并: "
time envcli run TEST=1 echo "test" >/dev/null 2>&1

# 测试 4: 列出所有变量
echo -n "列出变量: "
time envcli list >/dev/null 2>&1

echo "=== 测试完成 ==="
```

---

## 📊 预期成果

### 性能提升

| 操作 | 优化前 | 优化后 | 提升 |
|------|--------|--------|------|
| `envcli get VAR` | ~10ms | ~2ms | 80% |
| 100 次连续查询 | ~300ms | ~50ms | 83% |
| `envcli run` | ~50ms | ~20ms | 60% |
| 文件 I/O 次数 | 400+ | ~40 | 90% |
| 注册表读取 | 100+ | ~10 | 90% |

### 代码质量

- ✅ 保持 100% 测试通过率
- ✅ 0 Clippy 警告
- ✅ 代码简洁性不变
- ✅ 向后兼容

---

## 🚀 实施步骤

### Day 1: 系统环境缓存

**上午**:
1. 实现 `SystemEnvCache` 结构
2. 修改 `get_system_env()` 函数
3. 添加缓存控制函数

**下午**:
4. 编写单元测试
5. 性能对比测试
6. 代码审查和优化

### Day 2: 文件缓存

**上午**:
1. 实现 `FileCacheEntry` 和全局缓存
2. 修改 `Store::get_from_source()`
3. 修改 `Store::list_from_source()`

**下午**:
4. 优化 `merge_environment()`
5. 集成测试
6. 性能基准测试

### Day 3: 验证和优化

**上午**:
1. 运行完整测试套件
2. 性能对比分析
3. 修复发现的问题

**下午**:
4. 文档更新
5. 提交代码
6. 性能报告

---

## 📝 提交信息模板

```bash
git add src/utils/paths.rs src/core/store.rs src/utils/env_merge.rs
git commit -m "perf: 实现性能缓存优化

- 添加系统环境变量缓存 (60秒 TTL)
- 添加文件内容缓存 (基于修改时间)
- 优化环境变量合并算法
- 减少 80-90% 的文件 I/O 操作
- 减少 70-85% 的系统调用

性能提升:
- 单次查询: 80% 加速
- 100次查询: 83% 加速
- 环境合并: 60% 加速

所有测试通过，代码质量保持"
```

---

## ⚠️ 注意事项

### 1. 缓存一致性
- 文件修改后必须失效缓存
- 使用 `last_modified` 时间戳检测
- 提供手动清除缓存的方法

### 2. 并发安全
- 使用 `RwLock` 而非 `Mutex` (读多写少)
- 避免死锁
- 测试多线程场景

### 3. 内存使用
- 监控缓存大小
- 考虑添加缓存大小限制
- 定期清理过期条目

### 4. 错误处理
- 缓存读取失败时降级到原始方法
- 不影响核心功能

---

## 🎯 成功标准

### 性能指标
- [ ] 单次 `get()` < 5ms
- [ ] 100 次查询 < 100ms
- [ ] 文件 I/O 减少 80%+
- [ ] 系统调用减少 70%+

### 功能正确性
- [ ] 所有 324 个测试通过
- [ ] 缓存一致性 100%
- [ ] 并发安全
- [ ] 错误处理完善

### 代码质量
- [ ] 0 Clippy 警告
- [ ] 代码覆盖率 > 90%
- [ ] 文档完整
- [ ] 向后兼容

---

**开始日期**: 2025-12-30
**预计完成**: 2025-1-1
**负责人**: Claude Code AI
