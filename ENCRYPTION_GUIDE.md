# 🔒 EnvCLI 加密存储使用指南

> 使用 SOPS 对敏感环境变量进行加密存储

## 📋 目录

- [前置要求](#前置要求)
- [快速开始](#快速开始)
- [核心概念](#核心概念)
- [命令详解](#命令详解)
- [工作流程](#工作流程)
- [高级配置](#高级配置)
- [最佳实践](#最佳实践)
- [故障排除](#故障排除)

---

## 前置要求

### 1. 安装 SOPS

```bash
# macOS
brew install sops

# Linux (Ubuntu/Debian)
# 下载 release: https://github.com/mozilla/sops/releases
wget https://github.com/mozilla/sops/releases/download/v3.8.1/sops_3.8.1_amd64.deb
sudo dpkg -i sops_3.8.1_amd64.deb

# Windows
choco install sops
# 或使用 Scoop
scoop install sops
```

### 2. 配置加密后端

#### 选项 A: GPG（最简单，适合个人）

```bash
# 生成 GPG 密钥
gpg --generate-key

# 查看密钥 ID
gpg --list-secret-keys --keyid-format LONG

# 导出公钥（用于分享）
gpg --export --armor your@email.com > public.key
```

#### 选项 B: Age（推荐，现代且简单）

```bash
# 安装 age
# macOS
brew install age

# Linux
# 下载: https://github.com/FiloSottile/age/releases

# 生成密钥
age-keygen -o ~/.sops/age/key.txt

# 查看公钥（用于配置）
age-keygen -y ~/.sops/age/key.txt
```

#### 选项 C: 云服务（企业级）

- AWS KMS: 配置 AWS 凭证
- GCP KMS: 配置 GCP 服务账号
- Azure KMS: 配置 Azure 服务主体
- HashiCorp Vault: 配置 Vault 连接

### 3. 配置 SOPS

创建 `~/.sops.yaml` 配置文件：

```yaml
# 使用 Age（推荐）
creation_rules:
  - path_regex: .*
    age: age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p

# 或使用 GPG
creation_rules:
  - path_regex: .*
    pgp: "Fingerprint: 0x1234567890ABCDEF"
```

### 4. 验证安装

```bash
# 检查 SOPS
sops --version

# 检查 EnvCLI 集成
envcli check-sops
# 输出:
# ✓ SOPS 可用
# 版本: 3.8.1
```

---

## 快速开始

### 1. 加密并存储变量

```bash
# 进入项目目录
cd my-project

# 加密敏感变量（仅支持 local 层）
envcli encrypt DB_PASS my_secret_password

# 使用 set 命令加密
envcli set API_KEY sk-1234567890 --encrypt

# 详细模式
envcli encrypt DB_PASS secret --verbose
# 输出: ✓ 已加密并存储变量: DB_PASS
```

### 2. 查看加密状态

```bash
# 查看文件内容（加密变量显示为 ENC[SOPS:...]）
envcli export --source=local

# 输出示例:
# DB_HOST=localhost
# DB_PASS=ENC[SOPS:v1:...]
# API_KEY=ENC[SOPS:v1:...]
```

### 3. 自动解密读取

```bash
# get 命令自动解密
envcli get DB_PASS
# 输出: my_secret_password

# run 命令自动注入解密后的值
envcli run -- cargo run  # DB_PASS 会被自动解密
```

### 4. 手动解密

```bash
# 解密变量
envcli decrypt DB_PASS
# 输出: my_secret_password

# 解密指定层级
envcli decrypt API_KEY --source=local
```

---

## 核心概念

### 加密存储格式

加密后的变量在 `.envcli/local.env` 中：

```env
# 明文变量
DB_HOST=localhost
DB_PORT=5432

# 加密变量（SOPS 格式）
DB_PASS=ENC[SOPS:v1:...encrypted_data...]
API_KEY=ENC[SOPS:v1:...encrypted_data...]
```

### 加密类型检测

EnvCLI 自动检测变量是否加密：

```rust
// 检测逻辑
fn is_encrypted(value: &str) -> bool {
    value.starts_with("ENC[SOPS:") && value.ends_with(']')
}
```

### 数据结构

```rust
pub struct EncryptedEnvVar {
    pub key: String,
    pub value: String,              // 可能是加密的或明文的
    pub source: EnvSource,
    pub timestamp: u64,
    pub encryption_type: EncryptionType,  // None 或 Sops
}
```

---

## 命令详解

### 1. encrypt - 加密并存储

```bash
envcli encrypt <KEY> <VALUE> [OPTIONS]

# 示例
envcli encrypt DB_PASS secret123
envcli encrypt API_KEY sk-abc --verbose
```

**选项**:
- `--verbose, -v`: 显示详细输出

**限制**:
- 仅支持 `local` 层
- 需要 SOPS 已安装并配置

### 2. decrypt - 解密变量

```bash
envcli decrypt <KEY> [OPTIONS]

# 示例
envcli decrypt DB_PASS
envcli decrypt API_KEY --source=local
```

**选项**:
- `--source, -s <LEVEL>`: 指定来源层级

**行为**:
- 不指定 `--source`: 自动按优先级查找并解密
- 指定 `--source`: 从指定层级解密

### 3. set-encrypt - 设置并加密

```bash
envcli set-encrypt <KEY> <VALUE> [OPTIONS]

# 示例
envcli set-encrypt DB_PASS secret --encrypt
envcli set-encrypt DB_HOST localhost  # 明文存储
```

**选项**:
- `--encrypt, -e`: 使用 SOPS 加密存储

### 4. check-sops - 检查 SOPS 可用性

```bash
envcli check-sops

# 输出示例:
# ✓ SOPS 可用
# 版本: 3.8.1
```

### 5. list - 查看加密状态

```bash
# JSON 格式显示加密类型
envcli list --source=local --format=json

# 输出示例:
# [
#   {
#     "key": "DB_PASS",
#     "value": "ENC[SOPS:v1:...]",
#     "source": "local",
#     "timestamp": 1735200000,
#     "encryption_type": "Sops"
#   }
# ]
```

### 6. get - 自动解密

```bash
# 自动解密并输出明文
envcli get DB_PASS

# 详细错误
envcli get DB_PASS --verbose
```

### 7. run - 运行时自动解密

```bash
# 自动解密所有变量
envcli run -- cargo run

# 临时覆盖加密变量
envcli run DB_PASS=temp -- cargo run
```

---

## 工作流程

### 场景 1: 个人开发

```bash
# 1. 加密敏感配置
envcli encrypt DB_PASS dev_password
envcli encrypt API_KEY dev_key

# 2. 正常使用（自动解密）
envcli get DB_PASS
envcli run -- cargo run

# 3. 提交代码（安全！）
git add .envcli/project.env  # 可以提交
# local.env 已被 .gitignore 忽略
```

### 场景 2: 团队协作

```bash
# 开发者 A
envcli encrypt DB_PASS team_secret
git add .envcli/project.env
git commit -m "Add encrypted DB_PASS"
git push

# 开发者 B（克隆后）
git pull
envcli get DB_PASS  # 自动解密（需要配置相同密钥）

# 如果解密失败
envcli check-sops  # 检查 SOPS 配置
# 配置密钥后重试
```

### 场景 3: CI/CD 部署

```bash
# 1. CI 环境配置密钥
export SOPS_AGE_KEY_FILE=/secure/age/key.txt

# 2. 解密配置
envcli decrypt DB_PASS > /tmp/db_pass.txt
export DB_PASS=$(envcli decrypt DB_PASS)

# 3. 运行应用
envcli run -- npm start
```

### 场景 4: 多环境管理

```bash
# 开发环境
envcli encrypt DB_PASS dev_pass
envcli encrypt API_URL http://dev.api.com

# 测试环境（使用 project 层）
envcli encrypt DB_PASS test_pass --target=project
envcli encrypt API_URL http://test.api.com --target=project

# 生产环境（使用 project 层）
envcli encrypt DB_PASS prod_pass --target=project
envcli encrypt API_URL https://api.example.com --target=project
```

---

## 高级配置

### 团队共享加密

#### 1. 创建团队密钥配置

在项目根目录创建 `.sops.yaml`：

```yaml
creation_rules:
  - path_regex: \.envcli/(project|local)\.env
    age: >-
      age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p,
      age1lgg5xj2g3rjx4x4s4s4s4s4s4s4s4s4s4s4s4s4s4s4s4s4s4s4s4s4,
      age1xyz...
```

#### 2. 每个成员配置私钥

```bash
# 不要提交私钥！
echo "age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p" > ~/.sops/age/key.txt
chmod 600 ~/.sops/age/key.txt
```

#### 3. 加密团队共享变量

```bash
# 使用项目层（团队共享）
envcli encrypt DB_PASS team_secret --target=project

# 提交到 git
git add .envcli/project.env
git commit -m "Add encrypted team secret"
```

### 混合加密策略

```env
# .envcli/local.env (个人，不提交)
DB_HOST=localhost
DB_PASS=ENC[SOPS:...]  # 个人密码

# .envcli/project.env (团队，提交)
API_URL=https://api.example.com
API_KEY=ENC[SOPS:...]  # 团队 API 密钥
```

### 密钥轮换

```bash
# 1. 生成新密钥
age-keygen -o ~/.sops/age/key-new.txt

# 2. 更新 SOPS 配置
# ~/.sops.yaml
creation_rules:
  - path_regex: .*
    age: age1new-key...

# 3. 重新加密所有变量
envcli encrypt DB_PASS new_password
envcli encrypt API_KEY new_key

# 4. 分发新密钥给团队成员
```

---

## 最佳实践

### 1. 密钥管理

```bash
# ✅ 备份密钥
cp ~/.sops/age/key.txt ~/.backup/age-key-$(date +%Y%m%d).txt

# ✅ 使用密码管理器存储
# 1Password, Bitwarden, etc.

# ✅ 限制文件权限
chmod 600 ~/.sops/age/key.txt

# ❌ 不要做的事情
# 不要提交私钥到 git
# 不要将密钥硬编码在代码中
# 不要通过不安全渠道传输密钥
```

### 2. 加密策略

```bash
# ✅ 应该加密的
envcli encrypt DB_PASSWORD secret
envcli encrypt API_KEY sk-...
envcli encrypt JWT_SECRET ...
envcli encrypt ENCRYPTION_KEY ...

# ❌ 不需要加密的
envcli set DB_HOST localhost
envcli set DB_PORT 5432
envcli set APP_ENV production
```

### 3. Git 策略

```bash
# .gitignore
.envcli/local.env
.sops/age/key.txt
*.key
*.pem

# 可以提交的
.envcli/project.env  # 加密后
.sops.yaml           # 配置（不含密钥）
```

### 4. 安全检查

```bash
# 定期检查
envcli doctor --verbose

# 检查是否有未加密的敏感变量
envcli list --source=local --format=json | grep -v "Sops"

# 检查文件权限
ls -la ~/.sops/age/key.txt  # 应为 -rw-------
ls -la .envcli/local.env    # 应为 -rw-------
```

---

## 故障排除

### 问题 1: SOPS 未安装

```bash
# 症状
envcli encrypt DB_PASS secret
# ❌ 错误: SOPS 未安装或不在 PATH 中

# 解决
# 1. 安装 SOPS
# 2. 验证安装
sops --version
# 3. 重试
envcli encrypt DB_PASS secret
```

### 问题 2: 加密失败 - 无密钥

```bash
# 症状
envcli encrypt DB_PASS secret
# ❌ 错误: encryption failed

# 解决
# 1. 检查 SOPS 配置
cat ~/.sops.yaml

# 2. 检查密钥是否存在
ls -la ~/.sops/age/key.txt

# 3. 测试 SOPS
echo "test" | sops --encrypt --input-type binary /dev/stdin
```

### 问题 3: 解密失败 - 权限问题

```bash
# 症状
envcli decrypt DB_PASS
# ❌ 错误: permission denied

# 解决
# 1. 检查密钥权限
chmod 600 ~/.sops/age/key.txt

# 2. 检查配置文件权限
chmod 600 ~/.envcli/local.env
```

### 问题 4: 团队成员无法解密

```bash
# 症状
# 开发者 B 无法解密开发者 A 加密的变量

# 解决
# 1. 确保团队成员都有密钥
# 2. 检查 .sops.yaml 是否包含所有公钥
# 3. 重新加密（使用共享密钥）
envcli encrypt DB_PASS shared_secret
```

### 问题 5: 自动解密不工作

```bash
# 症状
envcli get DB_PASS
# ❌ 输出: ENC[SOPS:...]  # 没有解密

# 解决
# 1. 检查变量是否正确标记为加密
envcli list --source=local --format=json

# 2. 手动解密测试
envcli decrypt DB_PASS

# 3. 检查 get 命令实现
envcli get DB_PASS --verbose
```

### 问题 6: 性能问题

```bash
# 症状
# 加解密很慢

# 解决
# 1. 使用更快的加密后端（Age > GPG）
# 2. 仅加密必要的变量
# 3. 考虑缓存解密结果（如果频繁访问）
```

---

## 性能参考

| 操作 | 时间 | 说明 |
|------|------|------|
| 加密 (Age) | 10-20ms | 快速 |
| 加密 (GPG) | 50-100ms | 较慢 |
| 解密 (Age) | 5-10ms | 非常快 |
| 解密 (GPG) | 20-50ms | 中等 |
| 文件大小增加 | 2-3x | 取决于数据 |

**建议**:
- 仅对敏感变量加密
- 优先使用 Age 后端
- 避免频繁加解密同一变量

---

## 安全提醒

### ⚠️ 重要警告

1. **密钥丢失 = 数据丢失**
   - 没有密钥无法解密
   - 务必备份密钥

2. **密钥泄露风险**
   - 保护好私钥文件
   - 使用强密码保护

3. **加密不是万能的**
   - 运行时内存可能泄露
   - 进程间通信可能被截获
   - 结合其他安全措施使用

### 安全检查清单

- [ ] 密钥已备份到安全位置
- [ ] 密钥文件权限正确 (600)
- [ ] 配置文件权限正确 (600)
- [ ] `.gitignore` 包含敏感文件
- [ ] 团队成员都有必要密钥
- [ ] 定期轮换密钥
- [ ] 监控密钥访问日志

---

## 相关资源

- [SOPS 官方文档](https://github.com/mozilla/sops)
- [Age 加密工具](https://github.com/FiloSottile/age)
- [EnvCLI 主文档](README.md)
- [最佳实践](https://wiki.mozilla.org/Security/Key_Management)

---

**最后更新**: 2025-12-26
**版本**: v0.2.0
**状态**: ✅ 已完成
