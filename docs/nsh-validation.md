# NSH 迁移验证记录

尚未完成迁移或证明性能没有下降。旧版入口保持可用，以下证据按实际覆盖范围解释。

## 固定输入

- 旧版：`df1c0ef40cfc1743086c10a0d7ec63d3e69c006f`。
- NSH：0.4.0，源码 `e626a8dd8f8d8b277b9507cfd5ac74d0e67020f3`。
- 官方 macOS arm64 CLI archive SHA-256：`f63c3d57c31b28c9b34227aac81f099d1e0e61f1db552a6ec5127d114eaf59cb`，提取前已验证。
- 当前环境：Apple M3 Max、arm64、macOS 27.2（26B123）、Rust 1.98.1。

## Rust 应用骨架

`cargo test -p oshwhub-nsh --test shell`：2 通过。先在官方计数器模板观察到导航缺失失败，再实现 Rust 页面导航和外观状态；验证设置切换外观后跨页保留、关于/实验室/发现入口可达。此时发现与实验室仅有页面标题，尚未接入业务或查看器。

`cargo tree -p oshwhub-nsh -e normal,build`：没有 `nsh-js` 或 `rquickjs`。所有 NSH crate 锁定 =0.4.0；platform、engine 和 testing 关闭默认 JS 特性。

`cargo test --workspace --locked`：107 通过、3 忽略，包含原有 core/协议测试与 2 项新导航测试。`cargo clippy -p oshwhub-nsh --all-targets --locked -- -D warnings` 和 `cargo fmt --all --check` 通过。

`cargo run -p oshwhub-nsh` 成功启动原生桌面进程。官方 `nsh package --target macos` 生成 `.nsh/out/OSHWHub.app`；实际启动 Release 包，经系统可访问性树和截图确认中文页面、可选导航、设置/关于往返和即时深色主题渲染。截图保留在本地执行账本目录的 `task-1-native-dark.png`。这不是性能对照结果。

后续任务仍须验证页面业务、持久化、正式包 GPU 宿主、六平台功能和同口径 Release 性能。headless 导航测试不能替代这些验收。

## core 的 native / Web 通路（Task 2 进行中）

已将 reqwest/Tokio 保留在原生目标，浏览器使用 Rust `web-sys` Fetch/ReadableStream，保留 HTTPS 站点白名单、状态/MIME/JSON 校验、声明与流式字节上限。手动跳转模式拒绝跨站跳转，30 秒总超时和请求释放均通过 AbortController 取消 Fetch。浏览器不提供独立的连接阶段超时 API；总请求上限仍为 30 秒。

原生页面的强类型异步服务在 2 个 Tokio worker 上执行；不经过 FFI 或 JSON 调度。2 项服务测试确认无调用方 reactor 时仍能运行、保留 `INVALID_ID` 错误，以及释放请求会取消后台任务。计时改用 `web-time`，AES 解密关闭不需要的随机数功能，UUID/OBJ hash 的 Web 随机数显式配置。

`cargo check -p oshwhub-core --target wasm32-unknown-unknown`、原生和 WASM clippy `-D warnings` 通过。原生全工作区 109 通过、3 忽略。

官方 `nsh package --target web` 已生成完整 Rust Web 包（未压缩目录约 10.3 MB）。构建需要可用的 clang/llvm-ar；本机设置 `CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang`、`AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar`。wasm-bindgen、js-sys、web-sys 及 futures 固定为相互匹配的 0.2.129 / 0.3.106 / 0.4.79，CLI 必须为 0.2.129，通过 `NSH_WASM_BINDGEN` 指定。

`tests/nsh/playwright.config.ts` 下 9 项 Chromium 测试通过：7 项直接 core/WASM 边界与场景测试、2 项实际 Release NSH 包的响应式导航、主题保留与键盘操作。网络响应为浏览器路由夹具；流式分块单独使用可控 ReadableStream。它们不证明公共服务在线 CORS 可用，也不证明真实硬件渲染或性能。

尚未完成：全平台 GPU 表面，完整业务页面与查看器，旧存储迁移，在线 Web 通路及各平台性能。浏览器 CPU 场景准备当前在本地执行器执行，其大板延迟必须实测；若不满足基线须移到 worker，不能直接切换旧入口。
