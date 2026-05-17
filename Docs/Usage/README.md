# Glance 构建与部署

## 开发环境构建

### 前置条件

1. **Node.js** >= 18
2. **pnpm** >= 8
3. **Rust** >= 1.77 (通过 [rustup](https://rustup.rs/) 安装)
4. **Visual Studio Build Tools** — 勾选 "C++ 桌面开发"

### 安装

```bash
# 克隆仓库
git clone <repo-url>
cd Glance

# 安装前端依赖
pnpm install

# 检查 Rust 依赖
cd src-tauri && cargo check
```

### 开发模式运行

```bash
pnpm tauri dev
```

此命令启动：
- Vite 前端热重载服务器 (localhost:1420)
- Tauri WebView2 窗口
- 文件变更自动重编译

### 运行测试

```bash
# Rust 测试
cd src-tauri && cargo test

# 前端 Lint
pnpm lint

# 前端构建检查
pnpm build
```

---

## 生产构建

### 构建命令

```bash
pnpm tauri build
```

### 输出位置

```
src-tauri/target/release/bundle/
├── msi/              # Windows MSI 安装包
│   └── Glance_0.1.0_x64.msi
├── nsis/             # Windows NSIS 安装包
│   └── Glance_0.1.0_x64-setup.exe
└── Glance.exe        # 可执行文件
```

### 构建选项

| 选项 | 说明 |
|---|---|
| `--debug` | 调试构建 |
| `--target <triple>` | 交叉编译目标 |
| `--bundles <types>` | 指定打包类型 (msi, nsis) |

---

## Windows 安装包

### MSI 安装包

- 标准 Windows Installer 格式
- 支持静默安装：`msiexec /i Glance_0.1.0_x64.msi /quiet`
- 支持自定义安装路径

### NSIS 安装包

- 更小的安装包体积
- 自定义安装向导
- 支持卸载程序

---

## 配置文件

### 应用配置

位置：`%APPDATA%/Glance/config.json`

```json
{
  "theme": "system",
  "language": "zh-CN",
  "thumbnail_quality": 80
}
```

### Tauri 配置

位置：`src-tauri/tauri.conf.json`

关键配置项：
- `productName` — 应用名称
- `identifier` — 应用唯一标识
- `build.frontendDist` — 前端构建输出目录
- `build.devUrl` — 开发服务器地址
- `app.windows` — 窗口配置

---

## 环境变量

| 变量 | 说明 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | NSIS 安装包签名私钥 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码 |
| `APPLE_CERTIFICATE` | macOS 签名证书 |
| `APPLE_ID` | Apple 开发者账号 |

---

## 故障排除

### 编译错误

**Rust 编译失败**
```bash
# 清理并重新编译
cd src-tauri
cargo clean
cargo build
```

**前端构建失败**
```bash
# 清理依赖
rm -rf node_modules
pnpm install
```

### 运行时错误

**WebView2 未安装**
- Windows 10/11 通常已预装
- 手动安装：[WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

**数据库锁定**
- 检查是否有其他 Glance 实例运行
- 删除 `%APPDATA%/Glance/index.sqlite-wal` 和 `-shm` 文件

---

## CI/CD 集成

### GitHub Actions 示例

```yaml
name: Build
on: [push, pull_request]

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 18
      - run: pnpm install
      - run: pnpm tauri build
```

### 构建产物

- MSI/NSIS 安装包上传为 Release 资产
- 支持自动版本号（基于 git tag）
