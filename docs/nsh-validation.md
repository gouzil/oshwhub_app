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
