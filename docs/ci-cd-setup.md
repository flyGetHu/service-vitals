# CI/CD 构建配置说明

本项目已配置GitHub Actions自动化构建流程，支持在推送tag时自动构建和手动触发构建。

## 📋 功能特性

### 🏷️ Tag触发自动构建 (`build-on-tag.yml`)

**触发条件：** 推送以 `v` 开头的tag时自动执行

**构建流程：**
1. **代码检查**
   - 代码格式检查 (`cargo fmt`)
   - Clippy静态分析 (`cargo clippy`)
   
2. **测试执行**
   - 运行所有单元测试和集成测试
   
3. **Release构建**
   - 构建优化后的release版本二进制文件
   
4. **产物管理**
   - 上传构建产物到GitHub Artifacts
   - 自动创建GitHub Release
   - 包含构建信息文件

**支持的tag格式：**
- `v1.0.0` - 正式版本
- `v1.2.3-beta` - Beta版本（标记为预发布）
- `v2.0.0-alpha.1` - Alpha版本（标记为预发布）
- `v1.1.0-rc.1` - Release Candidate版本（标记为预发布）

### 🔧 手动触发构建 (`manual-build.yml`)

**触发方式：** 在GitHub仓库的Actions页面手动触发

**可配置选项：**
- **构建类型：** `debug` 或 `release`
- **运行测试：** 选择是否执行测试

## 🚀 使用方法

### 自动构建（推送tag）

```bash
# Linux/macOS (Bash)
# 1. 创建并推送tag
git tag v1.0.0
git push origin v1.0.0

# 2. 查看构建状态
# 访问：https://github.com/your-username/service-vitals/actions
```

```powershell
# Windows (PowerShell)
# 1. 创建并推送tag
git tag v1.0.0
git push origin v1.0.0

# 2. 查看构建状态
# 访问：https://github.com/your-username/service-vitals/actions
```

### 手动触发构建

1. 访问GitHub仓库的Actions页面
2. 选择 "Manual Build" 工作流
3. 点击 "Run workflow" 按钮
4. 选择构建参数：
   - 构建类型：debug/release
   - 是否运行测试：true/false
5. 点击 "Run workflow" 确认执行

## 📦 构建产物

### Tag构建产物
- **文件名格式：** `service-vitals-{tag}-linux-x86_64`
- **包含内容：**
  - `service-vitals` - 可执行二进制文件
  - `build-info.txt` - 构建信息
- **保留时间：** 30天

### 手动构建产物
- **文件名格式：** `service-vitals-{debug|release}-{commit-hash}`
- **包含内容：**
  - `service-vitals` - 可执行二进制文件
  - `manual-build-info.txt` - 构建信息
- **保留时间：** 7天

## 🔍 构建状态监控

### 构建成功指标
- ✅ 代码格式检查通过
- ✅ Clippy静态分析无警告
- ✅ 所有测试用例通过
- ✅ 编译成功无错误

### 常见问题排查

**构建失败可能原因：**
1. **格式检查失败**
   ```bash
   # 本地修复
   cargo fmt
   ```

2. **Clippy警告**
   ```bash
   # 本地检查
   cargo clippy -- -D warnings
   ```

3. **测试失败**
   ```bash
   # 本地测试
   cargo test
   ```

4. **编译错误**
   ```bash
   # 本地构建检查
   cargo build --release
   ```

## 🔧 配置自定义

### 修改构建触发条件
编辑 `.github/workflows/build-on-tag.yml` 中的 `on.push.tags` 部分：

```yaml
on:
  push:
    tags:
      - 'v*'          # 所有v开头的tag
      - 'release-*'   # 所有release-开头的tag
      - '!*-draft'    # 排除draft后缀的tag
```

### 添加多平台支持
如需支持其他平台，可修改 `runs-on` 和添加matrix策略：

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
runs-on: ${{ matrix.os }}
```

## 📈 性能优化

### 构建缓存
- 使用 `Swatinem/rust-cache@v2` 缓存Rust依赖
- 自动管理Cargo缓存和target目录

### 并行化
- 测试和构建步骤会自动利用多核心
- Cargo会并行编译依赖项

## 🔒 安全考虑

- 使用官方GitHub Actions
- 所有步骤都在Ubuntu环境中执行
- 使用 `GITHUB_TOKEN` 进行认证，无需额外配置
- 构建产物仅在授权用户可访问的仓库中存储

## 📞 技术支持

如遇问题，请检查：
1. GitHub Actions执行日志
2. 本地构建是否成功
3. Tag命名是否符合规范
4. 仓库权限设置是否正确 