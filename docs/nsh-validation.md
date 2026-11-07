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
最新结果为 **246 通过、3 忽略（26 个 suite，Release）**。workspace all-target Release
Clippy 已在 macOS 输入修复后通过；WASM 应用库 Clippy 在此前 SVG 修复后通过。

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
这些包不构成有效产物；Windows/Linux 必须在各自系统重新构建并运行。上面新增的有效 Linux 包是在 Linux 容器重新构建的产物；Windows 现已有真实 Windows 测试程序运行证据，但正式 NSH 包的本机打包/安装仍待完成。
仓库根目录的 `pnpm build:nsh <target>` 已拒绝跨主机桌面/iOS 打包，并按目标分开输出目录。

Android APK 已安装到 `emulator-5554` 并启动到 `com.longbridge.nsh.AppActivity`；系统无障碍树读到搜索、发现、实验室、设置、关于等中文控件。新的 APK 已实际选择暗色、force-stop 并重新启动，设置仍为暗色；测试后恢复跟随系统。该设备是 Wear OS 模拟器，仅证明启动、语义树和 NSH 私有存储持久化，不证明手机或真实设备 GPU/性能。另一模拟器 `emulator-5556` 因签名不匹配未更新，未卸载或清数据。

随后 Android 已接入自有 TextureView/JNI GPU 宿主，`aarch64-linux-android` check/Clippy 通过。独立 `com.gouzil.oshwhub.surfaceprobe` 安装在手机模拟器 `emulator-5556`，实际 Vulkan 首帧、resize、NSH overlay、20 次挂载/销毁与 PCB 工具栏缩放/拖动旋转通过；已人工查看三张板体截图。它与上面的 Wear OS 持久化测试是不同设备、不同覆盖范围，均不是 Android 真机性能证明。

Linux 已接入 GTK GLArea/GLES 外部 framebuffer，复用 Rust wgpu renderer，无 CPU 整板投影或像素读回。Ubuntu 容器中 `cargo test -p oshwhub-nsh --locked`、all-target Clippy 通过；`xvfb-run -a cargo test -p oshwhub-nsh --locked --test linux_surface -- --ignored` 为 **1/1**，检查实际三角形像素、上下方向、resize、圆角裁剪、上层控件和 20 次销毁。测试先后捕获了未接 GPU 和上下倒置，再修复通过；生成的应用级注册宏也在测试中使用。随后在实际 ELF 包中发现并修复实验室示例 3D 被误禁用的旧功能回退；新增 mounted 回归与模式选择测试先失败后通过，重建的原生包已实际查看 PCB 2D/3D、缩放与拖动旋转。该环境使用软件 GLES，只证明功能与生命周期。CI 已配置匹配主机的 macOS/Windows/Linux 构建及 Linux Xvfb 回归，但尚无远端 CI 结果。

Windows 已改为 NSH 同设备 D3D11 索引/实例绘制与 D2D bitmap 合成，不再走逐三角形 Canvas 投影。最初 DX12 共享纹理原型在 Parallels 实测失败：NSH 的 D3D11 使用 Parallels Display Adapter，而 DX12 只有 Microsoft Basic Render Driver，LUID 不匹配；因此没有强行切到软件 GPU。新实现复用 NSH 的 D3D11 context，通过 context state swap 恢复全部绘图状态，无逐帧 CPU 等待/读回，沿用同一 WGSL、20-byte 顶点、相机校验和 128 MiB 预算。

`cargo xwin check/clippy/test --no-run` 的 x86_64-pc-windows-msvc 目标均通过；Release 主程序也已交叉编译，`file` 确认为真正 PE32+ x86-64 GUI 可执行文件，尚不等同于 Windows 本机正式 NSH package 或安装/启动验收。两项显式 ignored 原生测试已传至 Windows 11 ARM64（26100.4652，Parallels 20.3，D3D11 feature level 11.0）实际执行为 **2/2**：三处绿色实例像素/几何共享、失败替换保留旧资源、固定尺寸 64 次相机更新与缓存重绘、隐藏暂停、clip/overlay、resize 和 20 次创建/销毁；另用真实 129.6 MB CPU 顶点/72 MB GPU buffer 复现并修复上传期间 frame/tick 竞态，以及大旧场景换小新场景且增大 target 的预算错误，均保留 RED→GREEN 日志。CI 已配置显式执行两项测试，并允许无硬件适配器的托管 runner 使用 WARP 作功能回归。这些是 x64 测试程序在 ARM64 Windows 系统兼容层的功能证据，不是 Windows 原生硬件性能对照。测试结束已恢复 VM 的 pause-idle 设置；恢复挂起曾因宿主磁盘空间不足失败，清理本任务可重建编译缓存后已成功恢复为 suspended；未改动共享目录或防火墙。

## core 的 native / Web 通路

已将 reqwest/Tokio 保留在原生目标，浏览器使用 Rust `web-sys` Fetch/ReadableStream，保留 HTTPS 站点白名单、状态/MIME/JSON 校验、声明与流式字节上限。手动跳转模式拒绝跨站跳转，30 秒总超时和请求释放均通过 AbortController 取消 Fetch。浏览器不提供独立的连接阶段超时 API；总请求上限仍为 30 秒。

原生页面的强类型异步服务在 2 个 Tokio worker 上执行；不经过 FFI 或 JSON 调度。2 项服务测试确认无调用方 reactor 时仍能运行、保留 `INVALID_ID` 错误，以及释放请求会取消后台任务。计时改用 `web-time`，AES 解密关闭不需要的随机数功能，UUID/OBJ hash 的 Web 随机数显式配置。

`cargo check -p oshwhub-nsh --lib --target wasm32-unknown-unknown --locked` 和
`cargo clippy -p oshwhub-core --target wasm32-unknown-unknown --locked -- -D warnings` 通过。

官方 `nsh package --target web` 已生成完整 Rust Web 包（本次 OSHWHub 目录约 12.5 MiB）。构建需要可用的 clang/llvm-ar；本机设置 `CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang`、`AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar`。wasm-bindgen、js-sys、web-sys 及 futures 固定为相互匹配的 0.2.129 / 0.3.106 / 0.4.79，CLI 必须为 0.2.129，通过 `NSH_WASM_BINDGEN` 指定。

`node scripts/nsh-web-tests.mjs` 最新完整生成两个 Web Release 包并运行 **26/26 Chromium 测试**：8 项 core/WASM
边界与场景、2 项发现页服务行为、3 项 shell 响应式/键盘（含 retained 隐藏页/模式的真实 Tab 隔离）、5 项 GPU surface（含 20 次挂载/销毁、示例 PCB 像素、工具栏/滚轮缩放及准备期间的相机输入），2 项二维缓存/Retina 回归，以及 6 项视口像素测试（390×844、844×390、768×1024、1024×768、1440×900、1920×1080），分别验证中文标签、SVG 内文字和嵌入 PNG。已人工查看竖屏和短横屏截图。
网络响应为浏览器路由夹具；流式分块单独使用可控 ReadableStream。应用的 WASM 客户端固定请求同源 `/fetch`，当前只有 loopback dev-server 实现它。必须先在 `apps/nsh` 执行 `nsh package --target web`，再从仓库根目录执行 `cargo run -p oshwhub-dev-server -- --web-root apps/nsh/.nsh/out/OSHWHub` 来提供 Web 包。旧 Web 同样依赖 `http://127.0.0.1:4318/bridge`，所以当前迁移保持原有本地服务范围；普通静态托管无法加载数据或图片，公共 Web 部署仍不在现有支持范围。这些夹具测试不证明所有真实硬件渲染/性能。

已只读取得公开项目 `2507dcb6e20646e29377e4abd4922bfa`（立创·泰山派3M RK3576 开发板）的完整 PCB/原理图场景，保存于忽略的执行目录。输入约 17.1 MB、35 段历史、281,715 条记录；PCB 场景为 6,402 批次和 946 条文字，原理图 MIPI_DSI 为 11 批次和 215 条文字。同进程还取得 757,702 顶点、1,935,429 索引的三维板体（约 645,143 三角形），938 个器件绑定；82 个模型中 81 个可读取，1 个官方未提供，失败响应单独保留。场景与模型原始 JSON 及 SHA-256 清单均已保存；网络/编译成功仅证明样本可取得，不是新旧应用性能对照。

`node --test scripts/nsh-performance.test.mjs` 为 **7/7**。比较器会拒绝缺样本、元数据不一致、软件 GPU、NaN/空值、哈希错误、未覆盖交互时长的帧记录、低于 95% 置信度以及手势期间非零编译/路径解析/mesh 上传；基线或候选的正向生命周期资源趋势均失败。当前只有比较器 fixture，没有合格的真实旧版/NSH Release 样本，不能输出性能通过结论。旧版基线已在独立 detached 工作树 `/Users/gouzi/.codex/worktrees/nsh-baseline/oshwhub_app` 固定到上述 commit，frozen/offline 安装、Web production build 和 core dev-server Release build 均通过。

Android 可见性回归使用保留尺寸的 `invisible()`，在旧宿主上稳定复现隐藏期间模型更新后恢复仍无三角形；TextureView 的自身/窗口可见性回调修复后，完整 smoke（含 20 次销毁、缩放/旋转、hide/update/show 的真实红色像素）通过，并人工查看恢复截图。`hidden()` 会改变尺寸并触发重画，先前用它得到的通过不计为此问题的复现。Web 实际 Tab 遍历也发现 retained 页面和旧查看模式仍能得到焦点；根容器同时隐藏布局/可见性后，新增回归从失败转为通过，页面和相机状态继续保留。

真实在线 Web 已使用 Apple M3 Max 的 ANGLE Metal 硬件 renderer，依次进入发现、RK3576 详情、BOM（197 种 / 885 元件）、PCB 2D、PCB 3D（81/82 模型，1 项官方未提供且有明确错误/重试）、原理图；已人工查看中文、图形和内嵌图片截图。此为 Release Web 包的在线功能证据，不是新旧性能对照。

## 真实场景的交互诊断（非性能验收）

同一 headless Chromium 156.0.8078.80、ANGLE Metal / Apple M3 Max、1440×900、DPR=1，
分别运行旧版和 NSH Release，每个场景连续滚轮 20 秒，两版未同时采样。输入固定为上述 RK3576
项目及 MIPI_DSI 原理图；对两版实际取得的 PCB、原理图和 3D 内容字段归一化比较相等，
SHA-256 分别为 `1a305fc62753a14c2e1aa8a1746de25382b0843c14e2729b0a917ba4c6b2e7ef`、
`ab5b8c73640613a9f96aebcdd6e53aaf21f0851d383abae36878b3d003778e0e`、
`97634ca48fddbaffe5a37c04a06cab93e6b918d34cbf3fe5ee2d1c43928a247d`。

| 场景 | 旧版 / NSH Task CPU（秒） | 旧版 / NSH 操作数 | 旧版 / NSH rAF p95（ms） | 旧版 / NSH long tasks |
| --- | --- | --- | --- | --- |
| 普通页面 | 0.994 / 1.080 | 172 / 176 | 16.7 / 16.7 | 0 / 0 |
| PCB 2D，修复前 | 1.154 / 19.330 | 172 / 119 | 16.7 / 149.9 | 0 / 117 |
| PCB 2D，有界缓存合成 | 1.154 / 0.907 | 172 / 174 | 16.7 / 16.8 | 0 / 0 |
| 原理图，有界缓存合成 | 1.231 / 0.853 | 172 / 174 | 16.8 / 16.7 | 0 / 0 |
| PCB 3D，修复任务生命周期后 | 0.580 / 0.563 | 172 / 174 | 16.7 / 16.8 | 0 / 0 |

PCB 2D 的 CPU profile 定位到每帧重建 3,240 个 Path2D、208,858 条路径操作。
原实现把相机绑定在 Canvas 的绘制命令中，NSH Web 因而随每次相机变化重放全板。
现在共享 Rust viewer 使用固定物理尺寸的 overview/detail Canvas，将相机 transform 绑定在绘制外，
180ms 空闲后再细化，已有 detail 覆盖且密度足够时复用；每张缓存限制 4096 边长、4,194,304 像素。
图层切换仍一次替换共享 Drawing，选择框单独绘制。挂起时取消手势并发布最终相机，恢复后可继续细化。
mounted 回归已验证手势不重建 Drawing、覆盖内复用 detail，以及拖动中隐藏再恢复后的细化。
Web 路径计数验证连续滚轮不调用 Path2D.closePath；DPR=2 实际像素回归验证 6.05 倍缩放锚点、
半透明填充色、nonzero 孔洞和 HTMLCanvas backing 像素上限。测试输入已校正 Chromium 在 DPR=2
时对 CDP wheel delta 的缩放，应用输入逻辑未改动。

这些数值只证明本次诊断中的二维严重卡顿已消除。rAF 不是 compositor presentation，操作数稍有差异，
普通页面单组 CPU 略高；尚无重复组、置信区间、完整 RSS/GPU、冷启动和生命周期样本。
当时 Mac 锁屏，因此使用 headless 诊断；该结果不能作为性能验收，也未输入严格比较器。
原始 JSON、CPU profile、输入清单和日志保存于忽略的 `.superpowers/sdd/nsh-migration-plan/`。

NSH 0.4.0 的 Windows Canvas 缓存为 D2D `ID2D1CommandList` 向量命令列表，随 transform 回放，
并非这里验证的 Web bitmap 缓存。Linux 使用 image surface/GSK texture，macOS 使用 content layer；
不能据 Web 结果宣称所有平台都只合成位图。Windows 真实大板二维须单独测量，若回放仍造成回退，
再增加最小 bitmap 适配并重新验证。上述 Windows 三维 D3D11/D2D bitmap 功能证据不覆盖这一二维问题。

## Web GPU 准备任务的生命周期回归

采集 trace 时进一步核对实际像素，发现真实 RK3576 项目在显示 81/82 模型后，工具栏选中状态
仍有响应，但缩放、器件显隐已不改变 GPU 图像；trace 全为 NO_DAMAGE。此前 PCB 3D 的
`0.361s` Task CPU 等数据因此撤回，不能据此比较性能或认定实际绘出了全部已下载器件。

根因是 `refresh` 在相机 tick 的 effect 下启动 GPU 准备任务，下一 tick 会按 NSH owner 语义取消
该任务；任务已取走 GPU 并设置 `preparing=true`，取消后没有机会归还，导致表面永久停止更新。
现在在 surface 创建时保留 owner，tick 和布局回调均在该 owner 下调度准备，使其跨相机更新存活，
同时仍在组件销毁时取消。原有循环继续合并最新尺寸/资源 revision，并在完成时绘制最新相机。

新增 Chromium 回归保留真实 GPU 分配及原生 popErrorScope 的结果，只延迟 Promise 完成；
加载板体后在准备中缩放，旧实现稳定失败（红色像素为 0），修复后板体出现且下一次缩放继续改变像素。
重新构建两份 Web Release 包后完整 26/26 通过；workspace 246/3、native all-target 与 WASM 库 Clippy、
TypeScript、72 Vitest + 32 Node 测试通过。聚焦独立审阅未发现新的 Critical/Important。重新打开同一在线项目并确认载入新 WASM `nsh.bd25e83cd8.wasm`，
81/82 模型完成后，实际截图中可见新增器件，滚轮和工具栏缩放分别改变图像；修复前两者均不改变像素。
新一组 20 秒 headless 3D 诊断 Task CPU 为 0.563 秒（旧版 0.580 秒）、174 次滚轮、rAF p95 16.8ms、
0 long tasks。它仍是单组诊断，不能证明差异显著或性能已通过。

## 原生 SVG 保真与有界栅格化

Android 实际像素回归发现原有 SVG 整张缺失：NSH 0.4.0 Android Canvas 明确拒绝 vector image，
其共享 SVG 解码器还会忽略 text 和嵌入 image。原生 viewer 现在用已固定版本的 resvg，
复用仓库的 Noto Sans SC 字体，保留中文和内嵌 PNG/JPEG/GIF/WebP；仍由 NSH Canvas 合成。
Web 继续使用浏览器 SVG 解码。此前一次 iOS 在线封面加载失败在同一包重启后恢复，
NSH 原生 image 解码器支持 percent/base64 data URI，不能把它归因为 URI 编码或宣称已经修复网络根因。

聚焦审阅提出的三项 Important 均按具体故障处理：固定 4 倍 PNG 在深缩放下损失细线，
现改为保留 SVG 解析树，按既有 overview/detail 的可见世界区域和物理像素网格重新绘制，
每片最多 4096 边长、4,194,304 像素，整次图片缓存总量最多 16,777,216 像素。
手势继续只更新外部相机变换，空闲后才生成细节；新 PNG 解码成功前显示已有概览。
旋转/镜像使用原场景变换，图片 viewport 单独裁剪；路径和字体无需随细化重新解析。

resvg 对损坏位图只记日志而不返回错误，所以 href resolver 先完整、有界解码并归一化 PNG，
CRC 错误或截断数据会使场景显式报错。内嵌图片总像素、解压/XML、绘制访问次数和中间图层分配
均有上限；中间图层估算沿用 NSH 0.4.0 的算法，并附 Apache 许可证。
文件、网络和嵌套 SVG 引用明确拒绝，不会静默省略。图片处理逐张等待并使用单个受限 blocking 槽位，
取消后当前有界操作可以收尾，但排队作业及后续图片不能再启动，Tokio 网络线程不执行栅格化。

新增 Rust 图片测试 8/8，包含中文/内嵌 PNG、单边尺寸/viewBox 比例、损坏/超大图片、
外部资源/超大滤镜、局部深缩放、旋转裁剪及 Drawing 中图片替换。
损坏 PNG 测试先失败，修复后通过；取消/队列测试在移除并发限制时失败，恢复后通过。
Android arm64 手机模拟器上的完整 smoke 通过：Vulkan、resize/overlay、20 次销毁、
3D 缩放/拖动/隐藏恢复，以及 2D 半透明、nonzero 孔洞、中文、SVG 图片和 125%/596% 缩放。
固定 PNG 的深缩放细线像素峰值只有 128，回归失败；自适应版本满足大于 240 的断言，
并人工查看了中文与细线截图。模拟器结果不计入真机性能门槛。

当前 workspace 为 246 通过、3 忽略（26 suites），native all-target Clippy 通过。
WASM 生产库使用 `--lib` 检查；额外尝试的 `--all-targets` 因现有原生 Headless/Tokio 测试宿主
不支持 WASM 而失败，日志保留，不能称为全目标通过。Web 行为以重新打包后的浏览器回归为准。

## 本机 macOS 验证优先级

用户已手动解锁 Mac，并要求先验证 macOS。最新 SVG 实现已重新生成 macOS、iOS Simulator、
Android universal（arm64-v8a / armeabi-v7a / x86_64）和 Linux aarch64 包；最终 Android
SurfaceProbe 完整 smoke 再次通过。Linux 包确认 ELF aarch64，iOS Simulator 包已安装并启动，
实际可见在线中文列表与封面。各目标 NSH package 串行运行，避免生成目录被不同主机覆盖。

macOS 最新 Release 包已实际进入在线 RK3576 项目、BOM（197 种 / 885 元件）、PCB 2D、
PCB 3D 和 MIPI_DSI 原理图。NSH 0.4.0 没有将普通滚轮交给应用的相机处理函数，现增加
仅捕获滚轮和 magnify 的画布局部 NSView，并通过 NSH engine queue 更新已有相机。
二维、原理图滚轮均实测从 100% 到 123%；二维拖动、键盘适应后放大到 125%、三维滚轮和
拖动旋转实际改变画面。切换模式后二维 125% 状态保留。三维可见 81/82 模型，1 项官方未提供。
新包原理图中文与内嵌 logo 可见；触控板 pinch 尚无实际验证。mounted 二维/三维输入回归
先失败后通过，workspace Release 246/3 ignored、all-target Release Clippy 通过。
原理图拾取曾错误进入 PCB 拾取缓存并提示场景过期，现由模式注入对应的 core picker 修复；
失活模式清除暂时错误提示而保留相机、图层和属性状态。回归验证通过，实际 Mac 包重建后需再做一次短回归。
首次 PCB 读取曾超时、点击重试后成功，封面也曾显示重试，不宣称网络根因已解决。

包 UUID `E7394F59-2D33-3454-B96B-FF162503E93A` 与 Release runner 相同。
旧版独立工作树已构建出 arm64 Electron Release `.app`，实际打开同一个 45,117 对象的 PCB。
两版没有同时运行。当前窗口尺寸仍需统一，CUA 相同页数不代表相同逻辑像素滚轮输入。
Instruments 静态试采可读 Activity Monitor；应用 presentation 关联与 GPU 内存口径尚待确认。
20 秒 GPU 试采因磁盘不足失败，未作为验收数据；已清理本任务可重建的跨平台编译缓存。
尚无合格的新旧 macOS 性能采样。
2026-11-08 解锁后 CUA 恢复。用固定 NSH 0.4.0 CLI 对提交
`0d565a734b1c22dc722f22524b32198876b6b0d3` 重新构建 macOS Release 包（34.2 秒），
可执行文件 UUID `E79494CE-DC98-3650-8E02-D045A14FAC73`，SHA-256
`80abc6cce04cb933531847f97c306d1b77717675fd44d921f736eb0bd9b7ead7`。
新包实际进入在线 RK3576/BOM，MIPI_DSI 原理图点选 U20 得到 COMPONENT/e69、SY7201ABC 属性，
未再显示 PCB 场景过期；中文、细线、内嵌 logo 可见。PCB 2D 普通滚轮从 100% 到 128%，
切到 3D 再返回仍为 128%；3D 实际可见 81/82 模型，滚轮与拖动改变相机。退出查看器后可重新打开。
触控板 pinch 和鼠标/键盘组合输入仍未在本次验证中实测。

增加 `scripts/nsh-process-sample.mjs`，以进程树累计 CPU 秒数和 RSS 采样，包含单位换算、PID 重用、
进程树变化、原始输出防覆盖和真实忙进程回归。三维诊断保存 66 个样本、65.285 秒墙钟、
6.99 秒 CPU、10.707% 平均 CPU、1,109,458,944 bytes 峰值 RSS；输入的三段工具调用有间隙，
第三段已在采样结束后，因此该文件只能用于进程诊断，不能算连续交互组或新旧性能对照。
原始文件和命令保存在本计划忽略的工作目录 `macos-head-*` 中。

Instruments `Core Animation FPS` 明确报告 macOS 不支持该指标；
Animation Hitches + Core Animation Commits 试采在结束处理时报告
`set-column: Argument 2 should be of type 'backtrace' but was passed on object of type 'tagged-backtrace'`，
该跟踪同样不用于验收。其 target updates 导出有 326 条 OSHWHub PID 记录，保留用于研究帧关联；
不能把全屏 compositor 帧当作应用呈现帧。后续采样避免这些已失败的 Instruments 组合。
优先 macOS 不替代其余平台的最终验收。

2026-11-08 继续检查旧版工作区大小窗口（截图为 3456×2168 像素）。
`macos-baseline-fullscreen-pcb3d-process.json` 的文件名保留当时的命名；它约 55 秒，
CPU 为 3.47 秒、整棵进程树峰值 RSS 为 1,798,291,456 bytes。
输入循环在 CUA 60 秒超时后被中断，完成动作数未知；它与此前 2400×1664 像素的新包窗口
也不匹配，因此两份进程诊断不能配对比较。后续菜单显示“进入全屏幕”，不能只凭截图尺寸
把该窗口认定为 macOS 独立全屏空间。

此前失败跟踪的任务专属 DTServiceHub PID 57292 仍持有约 11.8 GB 原始 ktrace。
已确认路径与生成时间，停止该持有进程并删除这份冗余 scratch；清理清单保存于
`macos-head-ktrace-cleanup.json`，已导出的 XML 和错误日志保留。

新增 4 秒、3 秒尾窗口的 Instruments 小规模试采，避免再次产生巨型跟踪。
Activity Monitor 配合 Frame Lifetimes、Core Animation Activity、Core Animation Server、
Hitches，以及 Hitches + Display 的组合均正常保存，单份约 25–38 MB。
这只验证记录/导出链路：Frame Lifetimes 导出 680 条系统帧，Core Animation Server
导出 3,438 条 WindowServer 区间；均不能直接作为 OSHWHub 呈现帧。
Hitches 虽包含 `hitches-updates` 和 `hitches-frame-lifetimes` 表，当前两组导出均为 0 行，
尚未验证目标进程与实际显示帧的关联，不据此计算应用 FPS 或丢帧。
随后不使用尾窗口，给 Hitches 增加 Frame Lifetimes 数据源，导出 264 条系统帧；再增加
Core Animation Commits、保持默认 `expensiveCommitSampling=0`，且不启用 Time Profiler，
4 秒跟踪也成功保存（约 26 MB、282 条系统帧），没有复现此前的 backtrace 解析失败。
两者目标更新仍为 0 行，须等可验证的输入恢复后再检查。对历史失败跟踪的只读结构诊断中，
326 条目标更新按 display/swap/surface 全部匹配到 323 个不同帧终点；这一关联也没有将
失败跟踪变成有效样本，帧终点是否对应实际应用呈现仍待核实。

这轮 CUA 能读取旧版无障碍树和窗口图像，但拖动、滚轮持续报告
`noWindowsAvailable`；重新选择应用、前置窗口和重连会话未恢复可验证的输入。
只读系统状态为 `IOConsoleLocked=false`，故没有把它再次归因为锁屏。
已请求用户将 OSHWHub 切到当前桌面并恢复普通窗口；在输入恢复前不继续记录交互组。
本轮只改验证记录，应用代码和 Release 包未变；进程采样工具加入后的最近完整
`pnpm test` 结果为 72 个 Vitest + 39 个 Node 测试通过。

## Android 原有主题偏好兼容（2026-11-08）

原版 Android 把 `themeMode` / `themeColor` 保存在 `oshwhub-settings` SharedPreferences。
新宿主在加载 `libnsh` 后、启动 Rust 根组件前通过 JNI 读取快照；NSH 已保存的值优先，
只有缺失字段才使用经过校验的旧值，写入仍走 NSH 私有存储，不改旧文件。
两个字段独立加载；旧源失败不会丢弃另一个已保存字段，也不会阻止后续保存。
Activity 重建会刷新快照，允许从前一次读取失败中恢复。

迁移选择和重建恢复回归均经历 RED→GREEN；独立复审指出的错误优先级问题也已复现、修复。
最终 `cargo test --workspace --release --locked` 为 249 通过、3 忽略（26 个 test-result 组）；
Release workspace all-target、Android arm64 lib、WASM lib Clippy（`-D warnings`）、fmt 和 diff 检查通过。
Java facade/D8 构建通过；实际 arm64 `libnsh.so` 已确认导出
`Java_com_longbridge_nsh_AppActivity_initializeLegacyPreferences`。
本轮生产 ID 的 debug-signed arm64 APK SHA-256：
`b881b6181f580cc7cb62710fda53fe42a79c6eafe9724fe9def324ae7a6db256`。

手机模拟器 `emulator-5560`（API 34、arm64）使用独立包 ID
`com.gouzil.oshwhub.thememigrationprobe` 运行同一产品 Rust 根组件和 Java 宿主，未覆盖原应用。
预置旧 dark/purple 后设置页实际选中暗色/紫色；改成浅色/橙色，force-stop 后重开仍选中两者，
旧 SharedPreferences 文件字节保持不变，进程日志无 JNI 链接/启动异常；重开截图已人工检查。
原始无障碍树、截图、导出表及构建日志保存在执行账本目录。
这是旧存储/JNI 的模拟器功能验证，不是 Android 真机性能验收。
Electron 旧 localStorage 的主题数据迁移仍待完成。

## 尚未验收的门槛

尚未完成：同一设备/OS/GPU/视口/输入下的旧版与 NSH Release 性能样本（冷启动、2D/原理图/3D 交互、CPU/RSS/GPU、20 次生命周期）；真实大板和真实器件模型的持续 FPS/内存；Android 真机以及 iOS/Windows/Linux 的真实硬件渲染、无障碍和生命周期验收；Windows 正式 NSH 包的本机打包/安装；原生各端中文与嵌套 SVG 图片的像素矩阵。旧 RN/Electron/Skia/Expo 入口必须继续保留，直到这些门槛全部通过。
