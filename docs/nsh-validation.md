# NSH 迁移验证记录

尚未完成迁移或证明性能没有下降。旧版入口保持可用，以下证据按实际覆盖范围解释。

## 固定输入

- 旧版：`df1c0ef40cfc1743086c10a0d7ec63d3e69c006f`。
- NSH：0.4.0，源码 `e626a8dd8f8d8b277b9507cfd5ac74d0e67020f3`。
- 官方 macOS arm64 CLI archive SHA-256：`f63c3d57c31b28c9b34227aac81f099d1e0e61f1db552a6ec5127d114eaf59cb`，提取前已验证。
- 当前环境：Apple M3 Max、arm64、macOS 27.2（26B123）、Rust 1.98.1。

## 当前实现与检查

`cargo check --workspace --locked`、`cargo fmt --all -- --check`、`git diff --check` 和
`cargo clippy --workspace --all-targets --locked -- -D warnings` 均通过。`cargo test --workspace --locked`
最新结果为 **230 通过、3 忽略（25 个 suite）**。workspace all-target 与 WASM 应用库
Clippy 均在审阅修复后重新通过。

`pnpm typecheck` 通过；直接运行 `./node_modules/.bin/biome check .` 最新检查 151 个文件无问题；
`pnpm test` 审阅修复后的结果为 **72 个 Vitest + 32 个 Node 测试通过**（包含 3 项打包主机保护测试）。
本机 `pnpm lint` wrapper 仍因 Biome 进程异常退出（OOM，退出码 254）；同一工作树的直接 Biome CLI 已检查通过，需在 CI/标准开发机复核 wrapper。

`cargo tree -p oshwhub-nsh -e normal,build` 没有 `nsh-js` 或 `rquickjs`。所有 NSH crate 锁定
=0.4.0；应用 crate、platform、engine 和 testing 关闭默认 JS 特性。

`nsh check` 已对 `apps/nsh` 和 `tests/nsh-surface` 通过。官方 NSH 0.4.0 package 已实际生成：

- macOS `.app`；
- iOS physical runner（未签名，不能作为设备安装证据）与 iOS Simulator `.app`；后者已安装并启动于 iPhone 17 simulator；
- Android debug-signed APK（`arm64-v8a` 与 `armeabi-v7a`，约 62.2 MiB）；
- Linux `.tar.gz` 与启动器：在 Ubuntu 24.04 aarch64 容器内由 NSH 0.4.0 构建，`file` 确认为 ELF aarch64；Xvfb 下已启动并加载在线项目列表；
- Web Rust 包（OSHWHub 约 12.5 MiB，SurfaceProbe 约 11.4 MiB）。

撤回此前 Windows/Linux 打包成功的记录：在 macOS 上运行 NSH 0.4.0 跨桌面目标打包，
会成功生成扩展名正确、内部却是 macOS 启动器的包。本次复现中 `.exe` 的 `file` 结果
为 `Mach-O 64-bit executable arm64`，其中不包含目标平台的 Rust 根组件。
这些包不构成有效产物；Windows/Linux 必须在各自系统重新构建并运行。上面新增的有效 Linux 包是在 Linux 容器重新构建的产物；Windows 尚无对应运行证据。
仓库根目录的 `pnpm build:nsh <target>` 已拒绝跨主机桌面/iOS 打包，并按目标分开输出目录。

Android APK 已安装到 `emulator-5554` 并启动到 `com.longbridge.nsh.AppActivity`；系统无障碍树读到搜索、发现、实验室、设置、关于等中文控件。新的 APK 已实际选择暗色、force-stop 并重新启动，设置仍为暗色；测试后恢复跟随系统。该设备是 Wear OS 模拟器，仅证明启动、语义树和 NSH 私有存储持久化，不证明手机或真实设备 GPU/性能。另一模拟器 `emulator-5556` 因签名不匹配未更新，未卸载或清数据。

随后 Android 已接入自有 TextureView/JNI GPU 宿主，`aarch64-linux-android` check/Clippy 通过。独立 `com.gouzil.oshwhub.surfaceprobe` 安装在手机模拟器 `emulator-5556`，实际 Vulkan 首帧、resize、NSH overlay、20 次挂载/销毁与 PCB 工具栏缩放/拖动旋转通过；已人工查看三张板体截图。它与上面的 Wear OS 持久化测试是不同设备、不同覆盖范围，均不是 Android 真机性能证明。

Linux 已接入 GTK GLArea/GLES 外部 framebuffer，复用 Rust wgpu renderer，无 CPU 整板投影或像素读回。Ubuntu 容器中 `cargo test -p oshwhub-nsh --locked`、all-target Clippy 通过；`xvfb-run -a cargo test -p oshwhub-nsh --locked --test linux_surface -- --ignored` 为 **1/1**，检查实际三角形像素、上下方向、resize、圆角裁剪、上层控件和 20 次销毁。测试先后捕获了未接 GPU 和上下倒置，再修复通过；生成的应用级注册宏也在测试中使用。随后在实际 ELF 包中发现并修复实验室示例 3D 被误禁用的旧功能回退；新增 mounted 回归与模式选择测试先失败后通过，重建的原生包已实际查看 PCB 2D/3D、缩放与拖动旋转。该环境使用软件 GLES，只证明功能与生命周期。CI 已配置匹配主机的 macOS/Windows/Linux 构建及 Linux Xvfb 回归，但尚无远端 CI 结果。

Windows 当前仍走 Rust NSH Canvas 三维回放（100,000 三角形上限），原生 wgpu 接线和真实 Windows 运行尚未完成。这条 fallback 不能替代 GPU 性能证据。

## core 的 native / Web 通路

已将 reqwest/Tokio 保留在原生目标，浏览器使用 Rust `web-sys` Fetch/ReadableStream，保留 HTTPS 站点白名单、状态/MIME/JSON 校验、声明与流式字节上限。手动跳转模式拒绝跨站跳转，30 秒总超时和请求释放均通过 AbortController 取消 Fetch。浏览器不提供独立的连接阶段超时 API；总请求上限仍为 30 秒。

原生页面的强类型异步服务在 2 个 Tokio worker 上执行；不经过 FFI 或 JSON 调度。2 项服务测试确认无调用方 reactor 时仍能运行、保留 `INVALID_ID` 错误，以及释放请求会取消后台任务。计时改用 `web-time`，AES 解密关闭不需要的随机数功能，UUID/OBJ hash 的 Web 随机数显式配置。

`cargo check -p oshwhub-nsh --lib --target wasm32-unknown-unknown --locked` 和
`cargo clippy -p oshwhub-core --target wasm32-unknown-unknown --locked -- -D warnings` 通过。

官方 `nsh package --target web` 已生成完整 Rust Web 包（本次 OSHWHub 目录约 12.5 MiB）。构建需要可用的 clang/llvm-ar；本机设置 `CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang`、`AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar`。wasm-bindgen、js-sys、web-sys 及 futures 固定为相互匹配的 0.2.129 / 0.3.106 / 0.4.79，CLI 必须为 0.2.129，通过 `NSH_WASM_BINDGEN` 指定。

`node scripts/nsh-web-tests.mjs` 最新完整生成两个 Web Release 包并运行 **23/23 Chromium 测试**：8 项 core/WASM
边界与场景、2 项发现页服务行为、3 项 shell 响应式/键盘（含 retained 隐藏页/模式的真实 Tab 隔离）、4 项 GPU surface（含 20 次挂载/销毁、示例 PCB 像素及工具栏/滚轮缩放），以及 6 项视口像素测试（390×844、844×390、768×1024、1024×768、1440×900、1920×1080），分别验证中文标签、SVG 内文字和嵌入 PNG。已人工查看竖屏和短横屏截图。
网络响应为浏览器路由夹具；流式分块单独使用可控 ReadableStream。应用的 WASM 客户端固定请求同源 `/fetch`，当前只有 loopback dev-server 实现它。必须先在 `apps/nsh` 执行 `nsh package --target web`，再从仓库根目录执行 `cargo run -p oshwhub-dev-server -- --web-root apps/nsh/.nsh/out/OSHWHub` 来提供 Web 包。旧 Web 同样依赖 `http://127.0.0.1:4318/bridge`，所以当前迁移保持原有本地服务范围；普通静态托管无法加载数据或图片，公共 Web 部署仍不在现有支持范围。这些夹具测试不证明所有真实硬件渲染/性能。

已只读取得公开项目 `2507dcb6e20646e29377e4abd4922bfa`（立创·泰山派3M RK3576 开发板）的完整 PCB/原理图场景，保存于忽略的执行目录。输入约 17.1 MB、35 段历史、281,715 条记录；PCB 场景为 6,402 批次和 946 条文字，原理图 MIPI_DSI 为 11 批次和 215 条文字。同进程还取得 757,702 顶点、1,935,429 索引的三维板体（约 645,143 三角形），938 个器件绑定；82 个模型中 81 个可读取，1 个官方未提供，失败响应单独保留。场景与模型原始 JSON 及 SHA-256 清单均已保存；网络/编译成功仅证明样本可取得，不是新旧应用性能对照。

`node --test scripts/nsh-performance.test.mjs` 为 **7/7**。比较器会拒绝缺样本、元数据不一致、软件 GPU、NaN/空值、哈希错误、未覆盖交互时长的帧记录、低于 95% 置信度以及手势期间非零编译/路径解析/mesh 上传；基线或候选的正向生命周期资源趋势均失败。当前只有比较器 fixture，没有真实旧版/NSH Release 样本，不能输出性能通过结论。

Android 可见性回归使用保留尺寸的 `invisible()`，在旧宿主上稳定复现隐藏期间模型更新后恢复仍无三角形；TextureView 的自身/窗口可见性回调修复后，完整 smoke（含 20 次销毁、缩放/旋转、hide/update/show 的真实红色像素）通过，并人工查看恢复截图。`hidden()` 会改变尺寸并触发重画，先前用它得到的通过不计为此问题的复现。Web 实际 Tab 遍历也发现 retained 页面和旧查看模式仍能得到焦点；根容器同时隐藏布局/可见性后，新增回归从失败转为通过，页面和相机状态继续保留。

真实在线 Web 已使用 Apple M3 Max 的 ANGLE Metal 硬件 renderer，依次进入发现、RK3576 详情、BOM（197 种 / 885 元件）、PCB 2D、PCB 3D（81/82 模型，1 项官方未提供且有明确错误/重试）、原理图；已人工查看中文、图形和内嵌图片截图。此为 Release Web 包的在线功能证据，不是新旧性能对照。

## 尚未验收的门槛

尚未完成：同一设备/OS/GPU/视口/输入下的旧版与 NSH Release 性能样本（冷启动、2D/原理图/3D 交互、CPU/RSS/GPU、20 次生命周期）；真实大板和真实器件模型的持续 FPS/内存；Android 真机以及 iOS/Windows/Linux 的真实硬件渲染、无障碍和生命周期验收；Windows Canvas fallback 的原生 wgpu 替换；原生各端中文与嵌套 SVG 图片的像素矩阵。旧 RN/Electron/Skia/Expo 入口必须继续保留，直到这些门槛全部通过。
