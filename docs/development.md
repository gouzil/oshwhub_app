# 开发、检查与构建

## 工具链

Node.js 24、pnpm 10.32.1、Rust 1.98.1（rust-toolchain.toml）、prek。移动界面采用 Expo 58 / React Native 0.88 / React 19.3；桌面采用 Electron 45 + React Native Web，基础 UI 采用 React Native Reusables / UniWind / Tailwind CSS 4，图形使用 Skia 2.14 / CanvasKit。依赖以锁文件为准。

```sh
pnpm install --frozen-lockfile
prek install
pnpm dev
```

开发预览在 http://127.0.0.1:5173；Rust 核心仅绑定 127.0.0.1:4318。它不是正式包的外部后端，不支持开放到公网。移动构建直接调用进程内 Rust 核心；Electron 桌面通过随包私有 stdio 子进程调用相同核心。

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

三维宿主使用与 SDK 58 匹配的 `expo-gl`；添加或升级后须重新 prebuild 并构建安装原生包，仅刷新 JS 不会添加 GLView。iOS 还需在 `apps/mobile/ios` 内运行 `pod install`。桌面没有新增原生 GL 依赖，复用 Electron Chromium 的 WebGL；不支持 WebGL 时可继续二维查看。

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

macOS 拒绝 Expo 向 `System Events` 发送 Apple 事件时会返回 `-1743`，即使构建、安装和模拟器启动都已成功。仓库通过 pnpm patch 修复 `@expo/cli@58.1.14`：仅在窗口操作遇到此权限错误时输出警告，继续用 `simctl` 打开应用；其他模拟器错误和应用启动错误仍会报出。补丁随 `pnpm install --frozen-lockfile` 自动应用，无需更改系统自动化权限。如果模拟器窗口没有出现，可手动打开 Xcode 的 Simulator / DeviceHub。升级 Expo CLI 时需重新核对补丁，并运行 `node --test scripts/expo-ios-launch.test.mjs`。

## 桌面

```sh
pnpm desktop
pnpm build:desktop
pnpm build:desktop --dir
pnpm test:desktop
# macOS 打包后的可执行文件，也可作为最后一个参数传入
pnpm test:desktop 'apps/desktop/release/mac-arm64/OSHWHub.app/Contents/MacOS/OSHWHub'
```

Electron 自带 Chromium；Windows 的 Rust 编译需要 MSVC 构建工具，macOS 需要 Xcode Command Line Tools，Linux 需要 Rust 的系统编译工具。构建在目标系统与架构原生执行，脚本校验 Rust target 和二进制 hash，拒绝将本机 sidecar 混入其他平台包。

构建产物在 `apps/desktop/release/`：macOS 为 DMG / ZIP，Windows 为 NSIS EXE，Linux 为 AppImage / DEB。`--dir` 仅生成可运行目录包。macOS 默认未签名，分发前需配置签名与公证；此命令不会发布。

`pnpm desktop` 先构建 Rust sidecar，再启动 Vite 和 Electron；退出时清理它们。生产包使用 `oshwhub://app/` 加载本地资源，不启动 HTTP 后端。渲染器只能使用 preload 暴露的两个方法，不能直接访问 Node。

CanvasKit WASM 随应用本地打包；启动不从 CDN 下载渲染代码。窗口最小宽度 320px，移动端安全区域与平板支持由同一 UI 处理。

`test:desktop` 默认运行已构建的本地 Web 与 Rust 产物；传入可执行文件时检查实际目录包。它使用临时用户目录，验证生产协议、preload 隔离、Rust 健康检查和示例、Tailwind 控件、字体资源与 2D/3D，截图写入 `.work/desktop-smoke/`。需要可用桌面和 WebGL；远程图片由占位图替换，Rust 的公开项目请求仍可联网，因此这不是完全断网测试。

组件扩展、Tailwind 主题与复用示例见 [ui.md](ui.md)。

## 日志与排障

Rust 事件使用 tracing，记录 requestId、耗时、数量、错误码，不记录请求正文、HTTP凭据或解密 key/iv。开发服务从标准输出查看 JSON 日志；桌面写 Electron `app.getPath("logs")`（macOS 默认为 `~/Library/Logs/OSHWHub/`，其他系统为应用 userData 下的 logs 目录）；移动写应用 cache。两端文件均为 `oshwhub-core.log`，启动时超过 8 MiB 的上一份轮换为 `oshwhub-core.previous.log`。panic 只记录源码位置和类别，不打印可能含输入数据的 payload。单次长会话的文件大小不在运行中轮换。

```sh
adb -s emulator-5554 logcat -s ReactNativeJS AndroidRuntime
adb -s emulator-5554 shell run-as com.gouzil.oshwhub cat cache/oshwhub-core.log
RUST_LOG=info cargo run -p oshwhub-dev-server
```

Rust 业务错误可使用界面重试；如果桌面子进程崩溃、输出协议损坏或请求超时，当前 transport 会关闭，需要重启应用后再操作。单个 stdio 响应上限为 128 MiB，超限会明确失败。

报告问题时附错误码、requestId、平台版本、工程 ID、可重复步骤和是否触发大小限制。不要附账号凭据或私有 PCB 正文。画布性能应同时记录实际工程的图元数、批次数、加载阶段耗时和设备；单个示例的结果不代表全部工程。

三维日志包括 Rust 的 `pcb.solid.complete`、`pcb.model.complete` 及 JS 的 `pcb.3d.ready/render_failed/model_failed`，可核对网格顶点、装配数量、GPU 缓存字节与失败原因。模型 API 被拦截时检查 Rust 错误码，不要将板体就绪等同为所有器件已下载。原生模拟器的渲染结果、浏览器软件 GPU 与真实硬件帧时需分别记录。
