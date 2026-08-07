#!/usr/bin/env bash
# 构建 Android APK（debug 自带调试签名，可直接安装测试）
# 仅构建 arm64（aarch64），覆盖主流现代手机，加快首次出包
set -e
cd "$(dirname "$0")"
source ./android-env.sh
# Gradle 依赖走国内 Maven 镜像（~/.gradle/init.gradle），需直连不走代理以加速
unset HTTP_PROXY HTTPS_PROXY http_proxy https_proxy
# 关闭增量编译，规避 rustc 增量缓存损坏导致的 ICE（no entry found for key）
export CARGO_INCREMENTAL=0
cd web
echo "=== tauri android build (debug, arm64) @ $(date) ==="
MSYS_NO_PATHCONV=1 node_modules/.bin/tauri android build --debug --apk --target aarch64
echo "=== BUILD EXIT: $? @ $(date) ==="
