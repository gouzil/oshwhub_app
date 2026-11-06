# OSHWHub · Rust NSH

全 Rust NSH 迁移应用，固定 NSH 0.4.0。当前仍在迁移，正式入口保留旧版，功能与性能验收见 [迁移设计](../../docs/nsh-migration.md) 和 [验证记录](../../docs/nsh-validation.md)。

```sh
cargo run -p oshwhub-nsh
cargo test -p oshwhub-nsh
```

桌面直接启动使用原生 AppKit / GTK / Win32。安装官方 NSH 0.4.0 CLI 后，可在本目录运行 `nsh check`、`nsh package --target macos`（以及其他目标）。移动端、Web 和 GPU 宿主必须以实际构建和运行结果为准。

`src/lib.rs` 导出 Rust `App` 和 `registry()`。页面、状态与导航使用 Rust；产品与 headless 测试关闭 NSH 的 JS 默认特性，不使用应用 TSX。Web 允许 NSH 自动生成的启动脚本。

生成的 runner、包与类型文件留在忽略的 `.nsh/`；不要提交这些产物。CLI 和 `nsh-*` crate 版本必须一致。
