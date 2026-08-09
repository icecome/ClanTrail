#!/usr/bin/env bash
# 构建 Android APK（debug 自带调试签名，可直接安装测试）
# 仅构建 arm64（aarch64），覆盖主流现代手机，加快首次出包
set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."
source "$SCRIPT_DIR/android-env.sh"
# Gradle 依赖走国内 Maven 镜像（~/.gradle/init.gradle），需直连不走代理以加速
unset HTTP_PROXY HTTPS_PROXY http_proxy https_proxy
# 关闭增量编译，规避 rustc 增量缓存损坏导致的 ICE（no entry found for key）
export CARGO_INCREMENTAL=0
cd web
# 首次构建需初始化 Android 工程（生成 gen/android）；已存在则跳过，避免覆盖已有自定义
if [ ! -d "src-tauri/gen/android" ]; then
  echo "=== tauri android init @ $(date) ==="
  MSYS_NO_PATHCONV=1 node_modules/.bin/tauri android init
fi
# init 之后、build 之前：注入 windowSoftInputMode 与全面屏 theme（幂等，重 init 后自动重新注入）
bash "$SCRIPT_DIR/patch-android.sh"
echo "=== tauri android build (debug, arm64) @ $(date) ==="
MSYS_NO_PATHCONV=1 node_modules/.bin/tauri android build --debug --apk --target aarch64
echo "=== BUILD EXIT: $? @ $(date) ==="
