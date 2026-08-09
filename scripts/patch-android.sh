#!/usr/bin/env bash
# 在 `tauri android init` 生成 gen/android 之后，向其增量注入（幂等，可重复执行）：
#   1) MainActivity 的 android:windowSoftInputMode="adjustResize"
#      —— 输入法弹出时只压缩 WebView 高度，不挤压/平移整页主界面
#   2) 沉浸式透明状态栏 + 刘海屏适配（全面屏，避免内容被状态栏/通知栏遮挡）
# 注意：gen/android 是自动生成目录，重跑 init 会被覆盖；本脚本由 build-android.sh
#       在每次 build 前调用，确保注入始终生效。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="$REPO_ROOT/web/src-tauri/gen/android/app/src/main/AndroidManifest.xml"
THEMES="$REPO_ROOT/web/src-tauri/gen/android/app/src/main/res/values/themes.xml"

if [ ! -f "$MANIFEST" ]; then
  echo "[patch-android] 未找到 $MANIFEST，请先执行 'tauri android init'"
  exit 1
fi

python3 - "$(cygpath -w "$MANIFEST")" "$(cygpath -w "$THEMES")" <<'PY'
import sys, re, os

manifest, themes = sys.argv[1], sys.argv[2]

# 1) windowSoftInputMode：仅注入首个含 android:name 的 <activity>
with open(manifest, encoding='utf-8') as f:
    s = f.read()
if 'windowSoftInputMode' not in s:
    def inject_activity(m):
        head = m.group(1)
        if 'android:name' in head and 'windowSoftInputMode' not in head:
            return head + ' android:windowSoftInputMode="adjustResize"' + m.group(2)
        return m.group(0)
    s2, n = re.subn(r'(<activity\b[^>]*?)(\>)', inject_activity, s, count=1)
    with open(manifest, 'w', encoding='utf-8') as f:
        f.write(s2)
    print(f'[patch-android] 已注入 windowSoftInputMode=adjustResize (n={n})')
else:
    print('[patch-android] windowSoftInputMode 已存在，跳过')

# 2) 沉浸式 theme：向每个 <style> 注入透明状态栏 + 刘海屏（若尚未注入）
cutout = 'windowLayoutInDisplayCutoutMode'
if os.path.exists(themes):
    with open(themes, encoding='utf-8') as f:
        t = f.read()
    if cutout not in t:
        items = (
            '        <item name="android:statusBarColor">@android:color/transparent</item>\n'
            '        <item name="android:navigationBarColor">@android:color/transparent</item>\n'
            '        <item name="android:windowLayoutInDisplayCutoutMode">shortEdges</item>'
        )
        def inject_style(m):
            body = m.group(0)
            if 'statusBarColor' in body or cutout in body:
                return body
            return body.replace('</style>', '\n' + items + '\n    </style>', 1)
        t2, n = re.subn(r'<style\b[^>]*>.*?</style>', inject_style, t, flags=re.S)
        with open(themes, 'w', encoding='utf-8') as f:
            f.write(t2)
        print(f'[patch-android] 已注入沉浸式 theme 到 {n} 个 style')
    else:
        print('[patch-android] 沉浸式 theme 已存在，跳过')
else:
    print(f'[patch-android] 未找到 {themes}，跳过 theme 注入')
PY

# 3) 修复 BuildTask.kt 的 runTauriCli 调用：init 默认生成的 listOf("tauri", ...)
#    在本机无全局 tauri 时会报 Cannot find module .../src-tauri/tauri。
#    改为指向本地 node_modules 的 tauri.js（幂等，重 init 后自动重新修复）。
BT=$(find "$REPO_ROOT/web/src-tauri/gen/android" -name "BuildTask.kt" 2>/dev/null | head -1)
if [ -n "$BT" ]; then
  if grep -q 'listOf("../node_modules/@tauri-apps/cli/tauri.js"' "$BT"; then
    echo '[patch-android] BuildTask.kt 已修复，跳过'
  else
    python3 - "$(cygpath -w "$BT")" <<'PYBT'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
s2 = s.replace(
    'listOf("tauri", "android", "android-studio-script")',
    'listOf("../node_modules/@tauri-apps/cli/tauri.js", "android", "android-studio-script")',
)
open(p, 'w', encoding='utf-8').write(s2)
print('[patch-android] BuildTask.kt 已修复 runTauriCli 调用')
PYBT
  fi
else
  echo '[patch-android] 未找到 BuildTask.kt，跳过'
fi

# 4) Gradle 分发镜像：默认 services.gradle.org 在部分网络下 SSL 校验失败，
#    改为腾讯云镜像（幂等）。仅替换分发下载主机，不改变文件名。
GW=$(find "$REPO_ROOT/web/src-tauri/gen/android" -name "gradle-wrapper.properties" 2>/dev/null | head -1)
if [ -n "$GW" ]; then
  if grep -q 'mirrors.cloud.tencent.com/gradle' "$GW"; then
    echo '[patch-android] gradle-wrapper 已指向腾讯镜像，跳过'
  else
    python3 - "$(cygpath -w "$GW")" <<'PYGW'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
s = s.replace('services.gradle.org/distributions', 'mirrors.cloud.tencent.com/gradle')
open(p, 'w', encoding='utf-8').write(s)
print('[patch-android] gradle-wrapper distributionUrl 已改为腾讯云镜像')
PYGW
  fi
else
  echo '[patch-android] 未找到 gradle-wrapper.properties，跳过'
fi

# 5) Gradle 仓库镜像（Kotlin DSL）：gen/android 的 *.gradle.kts 默认用 google()/mavenCentral()
#    直连 dl.google.com / repo1.maven.org，本沙箱不可达（连接超时），导致 AGP 与依赖解析失败。
#    全部替换为阿里云镜像（google / central）。注意 *.gradle.kts 是 Kotlin DSL，必须用
#    `maven { url = uri("...") }`，不能用 Groovy 的 `maven { url '...' }`（幂等，重 init 后自动重打）。
for gf in $(find "$REPO_ROOT/web/src-tauri/gen/android" -name "*.gradle.kts" 2>/dev/null); do
  if grep -qE 'google\(\)|mavenCentral\(\)' "$gf"; then
    python3 - "$(cygpath -w "$gf")" <<'PYGR'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
s2 = s.replace('google()', 'maven { url = uri("https://maven.aliyun.com/repository/google") }')
s2 = s2.replace('mavenCentral()', 'maven { url = uri("https://maven.aliyun.com/repository/central") }')
if s2 != s:
    open(p, 'w', encoding='utf-8').write(s2)
    print('[patch-android] 已替换 Gradle 仓库镜像: ' + p)
PYGR
  fi
done
echo '[patch-android] Gradle 仓库镜像检查完成'

# 6) 对齐已安装的 Android SDK：本机 SDK 仅含 build-tools;34.0.0 + platforms;android-34，
#    而 Tauri 模板默认 compileSdk=36 + AGP 8.11（需 build-tools 35.0.0 + platforms;android-36），
#    且 SDK Manager 拉取组件走不可达的 dl.google.com。故把 app 模块对齐到本地已装 SDK，
#    避免联网下载组件（幂等，重 init 后自动重打）。
APP_GRADLE="$REPO_ROOT/web/src-tauri/gen/android/app/build.gradle.kts"
if [ -f "$APP_GRADLE" ]; then
  if grep -q 'buildToolsVersion = "34.0.0"' "$APP_GRADLE"; then
    echo '[patch-android] app build.gradle.kts 已对齐 SDK 34，跳过'
  else
    python3 - "$(cygpath -w "$APP_GRADLE")" <<'PYAPP'
import sys, re
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
# compileSdk 36 -> 34（android {} 顶层）
s = s.replace('compileSdk = 36', 'compileSdk = 34')
# targetSdk 36 -> 34（defaultConfig 内）
s = s.replace('targetSdk = 36', 'targetSdk = 34')
# 若未显式声明 buildToolsVersion，则在 compileSdk 行后插入 34.0.0
if 'buildToolsVersion' not in s:
    s = s.replace('compileSdk = 34\n', 'compileSdk = 34\n    buildToolsVersion = "34.0.0"\n', 1)
open(p, 'w', encoding='utf-8').write(s)
print('[patch-android] 已将 app build.gradle.kts 对齐到 SDK 34 (build-tools 34.0.0)')
PYAPP
  fi
else
  echo '[patch-android] 未找到 app/build.gradle.kts，跳过 SDK 对齐'
fi

# 7) 离线对齐（最关键）：本机 SDK 仅含 build-tools;34.0.0 + platforms;android-34，
#    但 Tauri 模板默认 AGP 8.11（强制 build-tools 35.0.0）且 activity-ktx:1.10.1 /
#    lifecycle-process:2.10.0 要求 minCompileSdk 35，导致必须下载 build-tools 35 + platform 35。
#    本沙箱镜像无法拉到这两个组件，故降级 AGP 到 8.6.0（可用本地 build-tools 34 的最高版本），
#    并把高 minCompileSdk 的 androidx 依赖降到 34 兼容版本，使 compileSdk 34 完全离线可构建。
#    （幂等，重 init 后自动重打）
ROOT_GRADLE="$REPO_ROOT/web/src-tauri/gen/android/build.gradle.kts"
if [ -f "$ROOT_GRADLE" ]; then
  if grep -q 'com.android.tools.build:gradle:8.6.0' "$ROOT_GRADLE"; then
    echo '[patch-android] 根 build.gradle.kts 已降级 AGP 8.6.0，跳过'
  else
    python3 - "$(cygpath -w "$ROOT_GRADLE")" <<'PYAGP'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
s = s.replace('com.android.tools.build:gradle:8.11.0', 'com.android.tools.build:gradle:8.6.0')
open(p, 'w', encoding='utf-8').write(s)
print('[patch-android] 已降级 AGP -> 8.6.0')
PYAGP
  fi
else
  echo '[patch-android] 未找到根 build.gradle.kts，跳过 AGP 降级'
fi

# buildSrc 才是 AGP 版本的真实来源（Tauri 的 rust 自定义插件在此编译，app 模块 `id("com.android.application")`
# 无 version，实际 AGP 版本由 buildSrc 的 implementation 决定）。必须一并降级，否则仍解析到 8.11。
BUILDSRC_GRADLE="$REPO_ROOT/web/src-tauri/gen/android/buildSrc/build.gradle.kts"
if [ -f "$BUILDSRC_GRADLE" ]; then
  if grep -q 'com.android.tools.build:gradle:8.6.0' "$BUILDSRC_GRADLE"; then
    echo '[patch-android] buildSrc build.gradle.kts 已降级 AGP 8.6.0，跳过'
  else
    python3 - "$(cygpath -w "$BUILDSRC_GRADLE")" <<'PYBS'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
s = s.replace('com.android.tools.build:gradle:8.11.0', 'com.android.tools.build:gradle:8.6.0')
open(p, 'w', encoding='utf-8').write(s)
print('[patch-android] 已降级 buildSrc AGP -> 8.6.0')
PYBS
  fi
else
  echo '[patch-android] 未找到 buildSrc/build.gradle.kts，跳过 AGP 降级'
fi

if [ -f "$APP_GRADLE" ]; then
  if grep -q 'activity-ktx:1.9.0' "$APP_GRADLE"; then
    echo '[patch-android] app 依赖已降级到 compileSdk34 兼容版本，跳过'
  else
    python3 - "$(cygpath -w "$APP_GRADLE")" <<'PYDEP'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
s = s.replace('androidx.activity:activity-ktx:1.10.1', 'androidx.activity:activity-ktx:1.9.0')
s = s.replace('androidx.lifecycle:lifecycle-process:2.10.0', 'androidx.lifecycle:lifecycle-process:2.8.7')
open(p, 'w', encoding='utf-8').write(s)
print('[patch-android] 已降级 activity-ktx/lifecycle-process 到 compileSdk34 兼容版本')
PYDEP
  fi
else
  echo '[patch-android] 未找到 app/build.gradle.kts，跳过依赖降级'
fi

# 8) 对齐 Tauri 库模块(:tauri-android) 的 compileSdk：
#    tauri.settings.gradle 把 :tauri-android 模块的 projectDir 指向 cargo registry 里的
#    tauri-<ver>/mobile/android（第三方 crate 源码目录），其 build.gradle.kts 默认 compileSdk = 36，
#    导致 Gradle 求 platforms;android-36（本机只有 34）。gen/android 下并无该文件，故直接改 crate 源目录。
#    该 registry 解压目录在 cargo build 时不会被重解压覆盖，但加本步骤以应对 cargo clean 重解压场景。
for tf in $(find "$HOME/.cargo/registry/src" -path "*/tauri-*/mobile/android/build.gradle.kts" 2>/dev/null); do
  if grep -q 'compileSdk = 34' "$tf"; then
    echo "[patch-android] tauri crate build.gradle.kts 已对齐 compileSdk 34，跳过 ($tf)"
  else
    python3 - "$(cygpath -w "$tf")" <<'PYT'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
s2 = s.replace('compileSdk = 36', 'compileSdk = 34')
if s2 != s:
    open(p, 'w', encoding='utf-8').write(s2)
    print('[patch-android] 已将 tauri crate build.gradle.kts 对齐 compileSdk 34: ' + p)
PYT
  fi
done

# 9) Release 签名注入：Tauri 2.11 (CLI 2.11.4) 的 bundle.android 不支持 signingConfig 配置项，
#    且本环境无法从 tauri.conf.json 注入签名。改为在 Gradle 侧（app 模块的 release buildType）
#    内联 signingConfigs.create("release")，从环境变量读取 keystore 路径与密码（不硬编码敏感信息）。
#    build 前需 export:
#      CLANTRAIL_KEYSTORE_PATH / CLANTRAIL_KEYSTORE_PASSWORD / CLANTRAIL_KEYSTORE_ALIAS / CLANTRAIL_KEYSTORE_KEY_PASSWORD
#    （幂等，重 init 后自动重打）
APP_GRADLE="$REPO_ROOT/web/src-tauri/gen/android/app/build.gradle.kts"
if [ -f "$APP_GRADLE" ]; then
  if grep -q 'CLANTRAIL_KEYSTORE_PATH' "$APP_GRADLE"; then
    echo '[patch-android] app release 签名已注入，跳过'
  else
    python3 - "$(cygpath -w "$APP_GRADLE")" <<'PYSIGN'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
old = '''        getByName("release") {
            isMinifyEnabled = true'''
new = '''        getByName("release") {
            isMinifyEnabled = true
            signingConfig = signingConfigs.create("release") {
                storeFile = file(System.getenv("CLANTRAIL_KEYSTORE_PATH") ?: error("CLANTRAIL_KEYSTORE_PATH not set"))
                storePassword = System.getenv("CLANTRAIL_KEYSTORE_PASSWORD") ?: error("CLANTRAIL_KEYSTORE_PASSWORD not set")
                keyAlias = System.getenv("CLANTRAIL_KEYSTORE_ALIAS") ?: error("CLANTRAIL_KEYSTORE_ALIAS not set")
                keyPassword = System.getenv("CLANTRAIL_KEYSTORE_KEY_PASSWORD") ?: error("CLANTRAIL_KEYSTORE_KEY_PASSWORD not set")
            }'''
if old in s:
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8').write(s)
    print('[patch-android] 已注入 release signingConfig (env-based)')
else:
    print('[patch-android] 未找到 release 块锚点，跳过签名注入')
PYSIGN
  fi
else
  echo '[patch-android] 未找到 app/build.gradle.kts，跳过签名注入'
fi
