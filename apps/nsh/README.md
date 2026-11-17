# OSHWHub · Rust NSH

全 Rust NSH 迁移应用，固定 NSH 0.4.0。当前仍在迁移，正式入口保留旧版，功能与性能验收见 [迁移设计](../../docs/nsh-migration.md) 和 [验证记录](../../docs/nsh-validation.md)。

```sh
cargo run -p oshwhub-nsh
cargo test -p oshwhub-nsh
```

桌面直接启动使用原生 AppKit / GTK / Win32。安装官方 NSH 0.4.0 CLI 后，可在本目录运行 `nsh check`，并在仓库根目录使用 `pnpm build:nsh macos`（或 `windows`、`linux`、`ios`、`ios-simulator`、`android`、`web`）。输出位于 `.nsh/out/<target>/`；`NSH_BIN` 可指定 CLI 路径。桌面目标必须在对应系统构建，iOS 必须在 macOS 构建。NSH 0.4.0 跨桌面主机打包会生成错误平台的启动器，不能直接把该命令的成功当作产物通过。

`src/lib.rs` 导出 Rust `App` 和 `registry()`。页面、状态与导航使用 Rust；产品与 headless 测试关闭 NSH 的 JS 默认特性，不使用应用 TSX。Web 允许 NSH 自动生成的启动脚本。

生成的 runner、包与类型文件留在忽略的 `.nsh/`；不要提交这些产物。CLI 和 `nsh-*` crate 版本必须一致。

macOS/iOS/Android/Linux/Web/Windows 的三维查看器均已接入 GPU 表面：Metal、TextureView/JNI、GTK GLArea/GLES、WebGPU/WebGL，以及 Windows 与 NSH 共享的 D3D11/D2D 合成路径。Android 模拟器、Linux 软件 GLES 与 Windows D3D11 功能回归已验证像素/交互/销毁；真实设备性能对照通过前不会关闭旧入口。

Web 包当前必须由仓库中的同源本地服务提供。先在本目录运行 `nsh package --target web`，再在仓库根目录运行：

```sh
cargo run -p oshwhub-dev-server -- --web-root apps/nsh/.nsh/out/OSHWHub
```

该服务只监听 loopback；浏览器数据和图片请求都经过它的 `/fetch`。旧 Web 同样依赖本地 Rust 服务；迁移保持这一范围。普通静态托管不支持这些请求，公共 Web 部署尚未实现，不能把本地网络夹具测试当作在线网络验收。
