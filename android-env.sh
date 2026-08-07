#!/usr/bin/env bash
# Android 构建环境变量（Windows 原生路径给 Java/Gradle/Tauri，POSIX PATH 给 Git Bash）
# 用法: source android-env.sh
export JAVA_HOME='C:\Program Files\Microsoft\jdk-17.0.20.8-hotspot'
export ANDROID_HOME='C:\opt\workstations\project\apps\muji\tomb-keeper\android-sdk'
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export NDK_HOME='C:\opt\workstations\project\apps\muji\tomb-keeper\android-sdk\ndk\25.2.9519653'
export ANDROID_NDK_ROOT="$NDK_HOME"
export CARGO_HOME='C:\Users\bing\.cargo'
export RUSTUP_HOME='C:\Users\bing\.rustup'
export PATH="/c/Users/bing/.cargo/bin:/c/Program Files/Microsoft/jdk-17.0.20.8-hotspot/bin:$PATH"
