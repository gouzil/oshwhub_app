# Rust core 浏览器边界测试

这个测试专用 cdylib 直接调用产品 `oshwhub-core`。不使用旧 JSON HTTP sidecar，不作为应用产物交付。锁定 wasm-bindgen CLI 0.2.129，与 core 的 Web 依赖相同。

在仓库根目录准备探针，再执行浏览器测试：

```sh
cargo build --manifest-path tests/wasm-core/Cargo.toml --locked --target wasm32-unknown-unknown --target-dir target
wasm-bindgen target/wasm32-unknown-unknown/debug/oshwhub_wasm_core_probe.wasm --target web --out-dir output/nsh-web-tests --out-name probe
cp tests/wasm-core/index.html output/nsh-web-tests/index.html
pnpm exec playwright test --config tests/nsh/playwright.config.ts
```

需要 Rust WASM target、`wasm-bindgen-cli =0.2.129`、Python 3 及现有 Playwright Chromium。测试启动一个临时的 loopback 静态服务，网络响应通过浏览器路由夹具提供；流式上限测试另用可控 ReadableStream 决定分块。此处不验证公共服务的在线 CORS 配置，也不提供性能结论。
