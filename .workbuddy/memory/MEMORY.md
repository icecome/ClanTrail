# 项目长期记忆（tomb-keeper）

## Tauri v2 Android APK 构建配方（本环境已验证）

目标：`tauri android build --debug --apk --target aarch64` 出 arm64 调试 APK。

### 前置工具链（一次性）
- JDK 17：`C:\Program Files\Microsoft\jdk-17.0.20.8-hotspot`（Git Bash 用 `/c/Program Files/...` 风格 PATH）。
- Android SDK/NDK：`android-sdk/`（cmdline-tools/latest + platform-tools + platforms;android-34 + build-tools;34.0.0 + ndk;25.2.9519653），`sdkmanager --licenses` 接受全部。
- rustup 装好后：`rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`。
- `.cargo/config.toml`：4 个 android target 的 linker/ar 指向 NDK 25 的 clang 包装（`.cmd`）。

### 网络策略（关键）
- 海外源（services.gradle.org、gradle plugin portal、static.rust-lang.org）经沙箱继承的代理极慢/超时。
- 国内镜像直连高速：腾讯云 Gradle 镜像、`maven.aliyun.com/repository/{google,central,gradle-plugin}`。
- `~/.gradle/init.gradle`：用 `beforeSettings{ pluginManagement{ repositories{ 阿里云 gradle-plugin/google/central + gradlePluginPortal() } } }` + `allprojects{ repositories{ 阿里云 + google() + mavenCentral() } }`，绕过 Plugin Portal 卡点。
- `build-android.sh` 中 `unset HTTP_PROXY HTTPS_PROXY http_proxy https_proxy` 让 Gradle 走直连镜像。

### 必须的两个修复（已落到文件）
1. **rustc ICE**（增量缓存损坏导致 `no entry found for key`）：`build-android.sh` 设 `export CARGO_INCREMENTAL=0`，并清理 `target/aarch64-linux-android/debug/incremental`。
2. **rustBuildArm64Debug 报 "Cannot find module .../src-tauri/tauri"**：`gen/android/buildSrc/.../BuildTask.kt` 把 `listOf("tauri", "android", "android-studio-script")` 改为 `listOf("../node_modules/@tauri-apps/cli/tauri.js", "android", "android-studio-script")`（rootDirRel 指向 src-tauri 不动）。
   - ⚠️ `gen/android/` 是 `tauri android init` 自动生成的，重 init 会覆盖上述改动，需重新应用第 2 条。

### 运行方式
- 用 Bash 工具 `run_in_background: true` 托管长构建；**不要**在命令内 `nohup ... &`（会话结束子进程被回收，日志 0 字节）。

### 产物
- `web/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`（debug 签名，`adb install` 可测；仅含 arm64-v8a）。
- 体积大（unoptimized+debuginfo）；release 构建去 `--debug` 可瘦身。

## 内嵌后端 Tauri 桌面 APP
- 后端抽 `lib`（AppState/ApiError/build_router），Tauri `setup` 中 open SQLite + spawn Axum 监听 `127.0.0.1:8080`，DB/uploads 落 app data dir。
- `tauri.conf.json` 无 `app.android.permissions` 字段（v2 非法）；前端 `.env.tauri` 注入 `VITE_API_BASE=http://127.0.0.1:8080`。
- 桌面：`npx tauri build --debug --no-bundle` → `tomb-keeper/target/debug/tomb-keeper-tauri.exe`。

## 环境约定速记
- cargo 不在 PATH：rustup 代理缺失，用工具链原生路径 `C:\Users\bing\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin`（必须 Windows 原生路径，MSYS `/c/...` 原生子进程识别不了）。
- crates.io 超时：项目级 `.cargo/config.toml` 切 `rsproxy.cn` + sparse 协议。
- `sdkmanager.bat` 在 Git Bash 必须经 `cmd` 且用 `C:/...` 路径 + `MSYS_NO_PATHCONV=1`。
