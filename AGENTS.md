# AGENTS.md — clantrail

族迹 / 宗族谱系管理应用。桌面与 Android 双端，前端 React + Tauri v2 原生壳，壳内嵌 Axum 后端（SQLite）。

## 技术栈

- **前端**：React 18 + TypeScript + Vite 5 + TailwindCSS 3 + React Router 6；高德 JS API（地图/选点）。
- **后端**：Rust + Axum（REST API），SQLite（通过 `src-core` 的 ORM/模型层）。
- **桌面壳**：Tauri v2，`setup` 中启动内嵌 Axum 监听 `127.0.0.1:8080`，DB 与 uploads 落在应用私有目录。
- **移动端**：Tauri Android（`web/src-tauri/gen/android` 自动生成），arm64 为主。

## 目录结构

```
clantrail/
├── Cargo.toml              # workspace: [src-core, src-server, web/src-tauri]
├── src-core/               # 共享：数据模型、DB 访问、农历换算等（被 server / tauri 复用）
├── src-server/             # Axum REST API（独立后端二进制；也可被 Tauri 内嵌）
├── web/                    # 前端 + Tauri 壳
│   ├── src/                # React 源码
│   ├── src-tauri/          # Tauri 配置与 Rust 入口（lib.rs 内嵌后端）
│   ├── src-tauri/gen/      # ⚠️ 自动生成，勿手改（见下）
│   ├── package.json
│   └── .env.tauri          # 注入 VITE_API_BASE=http://127.0.0.1:8080
├── android-sdk/            # 本地 Android 工具链（gitignore，勿提交）
├── android-toolchain/      # 本地 JDK 17 工具链（gitignore，勿提交）
├── scripts/                # 构建脚本（android-env.sh / build-android.sh / patch-android.sh）
├── target/                 # Rust 构建产物（gitignore）
├── uploads/                # 运行时上传文件（gitignore）
├── backups/                # 运行时备份产物（gitignore）
└── apk/                    # 本地保留的构建产物 APK（gitignore）
```

## 常用命令

```bash
# 前端开发
cd web && npm install && npm run dev

# 前端构建（Tauri 打包前会先跑这个）
cd web && npm run build -- --mode tauri

# Rust 后端 / Tauri 编译
cargo build                # 含 src-core / src-server / tauri
cargo build -p clantrail-server
cargo test

# 桌面 APP（调试 EXE，跳过打包）
cd web && npx tauri build --debug --no-bundle

# Android APK（见下方「Android 构建注意事项」）
bash scripts/build-android.sh
```

## 架构约定

- **分层**：`src-core` 提供 `models` / `db` / 工具；`src-server` 暴露 HTTP；`web/src-tauri` 的 `lib.rs` 在 `setup` 里 `build_router(state)` + `spawn` 内嵌同样的后端。
- **状态/路由**：后端 `AppState`（含 `DbPool`、上传目录），`build_router(state)` 为 `pub`，便于被 Tauri 复用。
- **隐私克制**：调起外部地图只传坐标 + 墓地名称，不传任何逝者信息。
- **环境变量**：密钥/端口/路径从环境变量或 `.env` 读取，禁止硬编码（开发用 `.env`，生产注入）。

## 编码规范（与用户全局约定一致）

- TypeScript / Rust 类型优先，禁止 `any` / `interface{}` 兜底。
- 所有 `async` 必须有 try/catch；Rust 返回 `error` 必须检查。
- 用户输入拼 SQL 必须参数化（ORM/参数化查询），禁止字符串拼接。
- 项目代码禁用 `!important`、禁用 emoji。
- 注释精简，仅关键位置必要注释。
- 变更最小化：一次只做一件事；修复 Bug 不重构，重构不改功能。

## Android 构建注意事项（本环境已验证）

`bash scripts/build-android.sh` 会执行 `tauri android build --debug --apk --target aarch64`。前置与坑：

1. **工具链**：JDK 17 + `android-sdk/`（NDK 25.2.9519653）+ 4 个 Rust android target（`rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`）。`scripts/android-env.sh` 已设好 PATH/JAVA_HOME/ANDROID_HOME/NDK_HOME。
2. **网络**：海外源（gradle plugin portal、static.rust-lang.org）经代理极慢；国内镜像（阿里云 Maven、`~/.gradle/init.gradle`）直连快。`scripts/build-android.sh` 内 `unset` 代理走直连。crates.io 走 `.cargo/config.toml` 的 `rsproxy.cn` 镜像。
3. **rustc ICE**：增量缓存损坏会导致 `no entry found for key`，`scripts/build-android.sh` 已设 `CARGO_INCREMENTAL=0`。
4. **⚠️ 必须的一处改动**：`web/src-tauri/gen/android/buildSrc/.../BuildTask.kt` 由 `tauri android init` 自动生成。其 `runTauriCli` 中 `listOf("tauri", "android", "android-studio-script")` 需改为 `listOf("../node_modules/@tauri-apps/cli/tauri.js", "android", "android-studio-script")`，否则 `rustBuildArm64Debug` 报 `Cannot find module '.../src-tauri/tauri'`。**重跑 `tauri android init` 会覆盖此改动，需重新应用。**（`scripts/patch-android.sh` 会在每次构建前自动注入此修复及其余 Android 构建补丁）
5. **长构建用 Bash 工具 `run_in_background` 托管**；不要在命令内 `nohup ... &`（会话结束进程被回收）。
6. 产物：`web/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`（arm64，debug 签名，`adb install` 可测）。

## 数据库迁移

迁移由 `refinery` 管理（`src-core/migrations/V{n}__*.sql` 编译期嵌入，`ClanTrailDb::initialize` 自动执行）。`open()`/`reopen()`（导入备份后）都会自动迁移，老库与旧备份自动补列；persons 老库列由 `migrate_add_columns` 预守卫。`CURRENT_SCHEMA_VERSION`（`src-server/src/lib.rs`）= 最新迁移号，导出/导入 manifest 校验用。新增表/字段 = 新建迁移文件 + 同步更新导出兼容逻辑与 seed。
