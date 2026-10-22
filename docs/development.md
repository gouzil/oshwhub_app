# 开发、检查与构建

## 工具链

Node.js 24、pnpm 10.32.1、Rust 1.98.1（rust-toolchain.toml）、prek。移动界面采用 Expo 58 / React Native 0.88 / React 19.3；桌面采用 Tauri 2，图形使用 Skia 2.14 / CanvasKit。依赖以锁文件为准。

```sh
pnpm install --frozen-lockfile
prek install
pnpm dev
```

开发预览在 http://127.0.0.1:5173；Rust 核心仅绑定 127.0.0.1:4318。它不是正式包的外部后端，不支持开放到公网。移动和桌面构建直接调用进程内 Rust 核心。

## 检查

```sh
pnpm format
cargo fmt --all
pnpm check
prek run --all-files
pnpm test:e2e
```

在线 API 冒烟与离线测试分开运行，见 [api.md](api.md)。不要提交账号、Cookie、私有工程或解密日志。CodeGraph 已经用户授权，可运行 `codegraph init -i` 初始化；本地 `.codegraph` 不提交。

## Android

需要 JDK 21、Android SDK/NDK、CMake。设置 `ANDROID_HOME` 指向 SDK（macOS 默认探测 `~/Library/Android/sdk`）。Rust 目标：

```sh
rustup target add aarch64-linux-android x86_64-linux-android
node scripts/build-rust.mjs android
pnpm --filter @oshwhub/mobile exec expo prebuild --platform android
pnpm android
```

本机验证目标是 `emulator-5554`。Rust 脚本构建 arm64-v8a 与 x86_64，使用 NDK clang 并指定 16 KiB ELF page alignment。Gradle preBuild 会再次执行同一脚本，Cargo 复用已有结果。

原生桥需要 development build；Expo Go 不含本地 Rust 模块。生成的 Android/iOS 工程不提交，由 Expo prebuild 重建。CI 的 APK 使用开发签名，分发正式版本前必须配置自己的 release keystore。

Release 在生成原生工程后构建：

```sh
cd apps/mobile/android
./gradlew :app:assembleRelease --no-daemon
```

Expo 配置插件为 JS 打包任务补充仓库 `packages/`、`modules/` 的源码及共享 `assets/` 输入。这样共享界面修改会使增量 Release 重新打包，而不会误用旧 JS；构建缓存不包含 node_modules 或原生 build 目录。

## iOS / iPadOS

需要 macOS、Xcode 和 CocoaPods。

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
node scripts/build-rust.mjs ios
pnpm --filter @oshwhub/mobile exec expo prebuild --platform ios
pnpm ios
```

脚本把设备 arm64 与 simulator arm64/x86_64 静态库组装为 XCFramework，Podspec 随本地模块链接。simulator 与设备不能混为同一普通静态库。CI 构建未签名模拟器 app；真机安装与 App Store 发布需要开发者签名。

## 桌面

```sh
pnpm desktop
pnpm --filter @oshwhub/desktop tauri build
```

Linux 需要 WebKitGTK 4.1、appindicator、librsvg 和 patchelf；Windows 需要 MSVC 构建工具和 WebView2。macOS 使用系统 WKWebView。Tauri 构建产物在 `target/release/bundle/`。

CanvasKit WASM 随应用本地打包；启动不从 CDN 下载渲染代码。窗口最小宽度 320px，移动端安全区域与平板支持由同一 UI 处理。

## 日志与排障

Rust 事件使用 tracing，记录 requestId、耗时、数量、错误码，不记录请求正文、HTTP凭据或解密 key/iv。开发服务从标准输出查看 JSON 日志；桌面写 Tauri `app_log_dir`（macOS 为 `~/Library/Logs/com.gouzil.oshwhub/`，Windows 为应用本地数据的 logs 目录，Linux 为应用数据的 logs 目录）；移动写应用 cache。两端文件均为 `oshwhub-core.log`，启动时超过 8 MiB 的上一份轮换为 `oshwhub-core.previous.log`。panic 只记录源码位置和类别，不打印可能含输入数据的 payload。单次长会话的文件大小不在运行中轮换。

```sh
adb -s emulator-5554 logcat -s ReactNativeJS AndroidRuntime
adb -s emulator-5554 shell run-as com.gouzil.oshwhub cat cache/oshwhub-core.log
RUST_LOG=info cargo run -p oshwhub-dev-server
```

报告问题时附错误码、requestId、平台版本、工程 ID、可重复步骤和是否触发大小限制。不要附账号凭据或私有 PCB 正文。画布性能应同时记录实际工程的图元数、批次数、加载阶段耗时和设备；单个示例的结果不代表全部工程。
