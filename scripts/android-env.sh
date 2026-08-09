#!/usr/bin/env bash
# Android 构建环境变量（Windows 原生路径给 Java/Gradle/Tauri，POSIX PATH 给 Git Bash）
# 用法: source android-env.sh
export JAVA_HOME='D:\Develop\project\ClanTrail\android-toolchain\jdk-17.0.20+8'
export ANDROID_HOME='D:\Develop\project\ClanTrail\android-sdk'
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export NDK_HOME='D:\Develop\project\ClanTrail\android-sdk\ndk\25.2.9519653'
export ANDROID_NDK_ROOT="$NDK_HOME"
export CARGO_HOME='C:\Users\libing\.cargo'
export RUSTUP_HOME='C:\Users\libing\.rustup'
export PATH="/c/Users/libing/.cargo/bin:/c/Develop/project/ClanTrail/android-toolchain/jdk-17.0.20+8/bin:$PATH"

# Release 签名（Gradle 配置阶段强制求值，debug 构建也需要）
export CLANTRAIL_KEYSTORE_PATH="$(cd "$(dirname "$0")/.." && pwd)/android-keystore/release.keystore"
export CLANTRAIL_KEYSTORE_PASSWORD="ClantrailRelease2026"
export CLANTRAIL_KEYSTORE_ALIAS="clantrail"
export CLANTRAIL_KEYSTORE_KEY_PASSWORD="ClantrailRelease2026"

