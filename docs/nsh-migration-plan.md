# 全 Rust NSH 迁移 Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 全部页面与 PCB/原理图/三维渲染切换到 Rust NSH，并用对应平台的 Release 对照证明性能没有下降。

**Architecture:** 一个纯 Rust NSH 根组件，直接调用 `oshwhub-core` 的强类型接口。NSH owns UI/layout/accessibility；Rust owns scene/cache/GPU buffers。macOS/iOS/Android/Linux/Web 使用已接入的 GPU 表面；Windows 当前使用 Rust Canvas fallback，待原生 GPU 接线和真实硬件对照通过后替换。

**Tech Stack:** Rust 1.98.1、NSH =0.4.0、现有 Rust core、NSH Canvas、与 NSH 兼容的 wgpu、已有 Playwright 与原生平台测试。

**Spec:** `docs/nsh-migration.md`（用户 2026-11-06 明确选择全 Rust 方案）。

## Global Constraints

- 覆盖 Android、iOS、macOS、Windows、Linux 和 Web；保留当前全部公开只读功能。
- 禁止最终依赖 Electron、React Native、Skia、Expo GL 或应用 TSX/QuickJS；Web 自动生成的启动/wasm-bindgen 胶水除外。
- 固定 NSH =0.4.0，对照产品 commit `df1c0ef40cfc1743086c10a0d7ec63d3e69c006f`。
- URL 白名单、超时、下载/历史/解压/图元/GPU 上限、AES 验证、错误恢复和无障碍不得削弱。
- 手势只更新相机等小状态；整板编译、路径解析与 mesh 上传次数均为 0。
- 每项性能中位数/p95 等按设计逐项与旧版对照，不预留 5%/10% 的降级比例；噪声内无结论不能算通过。
- 不部署、不发布应用商店、不修改远端服务。旧运行时保留至全量验收通过。

## Review Focus

- 浏览器 `nativeComponent` 为空，不能把桌面注册成功当 Web 3D 支持；Task 2 必须检查真实像素、裁剪、焦点与销毁。
- 旧主题存储迁移失败不能覆盖用户选择；Task 5 检查损坏值、存储拒绝及重复迁移。
- 场景在切换页面/2D/3D 后异步完成，不能覆盖新页面或使用已销毁 GPU；Tasks 3/4/5 检查交错响应与释放。
- 密集真实 PCB 的图元/实例规模不能被小夹具掩盖；Tasks 3/4/6 用真实重放与独立合成压力样本分别报告。
- OS/GPU/浏览器兼容范围不得悄悄降低；Tasks 2/6 记录旧版与 NSH 的版本下限并实际验证，不支持时明确未通过。

## Current status (2026-11-07)

Rust NSH 页面、core/WASM 通路、二维/原理图 viewer、macOS/iOS/Android/Linux/Web 三维 GPU viewer、Windows Canvas fallback、业务页面与性能比较器已在工作树实现。workspace tests 最新为 230 通过/3 忽略，workspace/WASM/Linux Clippy 与 NSH check 通过；Web 最新 23/23 回归包含六视口中文与 SVG text/image 像素。Android Vulkan probe 已在手机模拟器验证缩放/旋转与 20 次销毁；Linux GTK GLES 像素/方向/叠层/resize/20 次销毁通过，Ubuntu 容器已生成真正 ELF aarch64 包并启动加载在线列表。此前 macOS 跨桌面生成的 Windows/Linux 假产物记录已撤回；新 Linux 包由 Linux 本机构建，Windows 仍需 Windows 实际构建/运行。最终迁移仍未验收：没有真实旧版 vs NSH Release 性能样本，Windows 原生 GPU 未接入，各端真实硬件/无障碍矩阵未完成。真实在线 Web 已走通发现/详情/BOM/PCB 2D/3D/原理图，并明确显示 1 项不可读取的官方模型。旧入口继续保留。

## File Structure

- `apps/nsh/nsh.json`、`Cargo.toml`、`theme.json`、`src/lib.rs`：产品声明、版本、主题与纯 Rust 根组件。
- `apps/nsh/src/main.rs`：直接运行桌面原生窗口；NSH CLI 管理其他平台 runner。
- `apps/nsh/src/components/`、`src/pages/`、`src/model.rs`：共享控件、页面与可独立测试的业务状态。
- `apps/nsh/src/viewer/`：二维/原理图、相机、缓存、三维 renderer 和平台表面。
- `apps/nsh/tests/`：真实 Rust NSH headless/模型/渲染边界测试。
- `crates/core/src/client.rs`、`lib.rs`、`pcb.rs`、`schematic.rs` 及各计算模块：仅为复用强类型 API、WASM 与共享数据所有权做必要适配。
- `scripts/`、`tests/e2e/`、`.github/workflows/ci.yml`：性能采集/比较、Web 回归与各端产物。
- `docs/nsh-validation.md`：命令、版本、输入、原始样本与所有验收缺口。

### Task 1: 可测试、无 QuickJS 的 Rust NSH 应用

**Files:** Create `apps/nsh/{nsh.json,Cargo.toml,theme.json,src/lib.rs,src/main.rs,src/components/mod.rs,tests/shell.rs}`; modify root `Cargo.toml`、`Cargo.lock`、`.gitignore`；create `docs/nsh-validation.md`。

**Interfaces:** Consumes `nsh_core::element::Root`、现有 core 类型；produces `oshwhub_nsh::App` 根组件、`registry()`、`cargo run -p oshwhub-nsh` 与 `nsh.json` Rust runner 配置。

- [x] 校验 NSH release archive 的 SHA-256 后用其 Rust 模板建立工程；所有 NSH 依赖精确锁定 =0.4.0，engine/platform/testing 均关闭 JS 默认特性。
- [x] 对官方初始 app 添加 headless 测试：打开设置、切换外观、进入关于并返回，验证只更新对应状态和可访问导航。Run `cargo test -p oshwhub-nsh --test shell`；Expected: 因产品导航尚不存在而失败。
- [x] 实现真实 Rust shell、共享 Button、语义主题 token、关于/实验室入口和外观状态绑定；后续页面的未实现功能不伪装成功。
- [x] Run `cargo test -p oshwhub-nsh --test shell`；Expected: 通过；Run `cargo tree -p oshwhub-nsh -e normal,build`；Expected: 无 `nsh-js`/`rquickjs`。
- [x] Run `cargo run -p oshwhub-nsh`；Expected: 真实原生窗口、可点击导航与主题；保存截图和实际进程信息。
- [x] Run `cargo fmt --all --check`、`cargo test --workspace --locked`；Expected: 新旧 Rust 测试全通过；显式暂存此任务文件并提交。

### Task 2: 全平台宿主与强类型 core / WASM 通路

**Files:** Modify `crates/core/{Cargo.toml,src/client.rs,src/lib.rs,src/pcb.rs,src/schematic.rs}`；create `apps/nsh/src/{services.rs,viewer/surface.rs,viewer/platform/}`、必要 Android NSH 宿主；tests `apps/nsh/tests/services.rs`、`surface.rs`。

**Interfaces:** Consumes Task 1 root and public `Api`/`HttpClient`/`Scene`/`Scene3d`；produces reusable service calls returning existing `Result<T>`、`Scene`/mesh ownership and a working GPU surface tied to NSH layout/disposal on each platform。

- [ ] 先编译 `nsh check` 和 `nsh package --target web`，记录真实失败；对损坏 ID、无限/超大拾取、下载重定向/超限与过期响应写失败用例。Expected: 平台边界缺失或新接口测试失败，保留既有安全测试。
- [ ] 做最小 target-specific 运行时、UUID、计时和网络拆分；原生继续复用 reqwest/Tokio，Web 使用合法浏览器网络路径；禁止将网络/计算搬回 JS 业务层。
- [ ] 验证 macOS/iOS/Windows/Linux 注册、Android 自有 NSH 宿主，以及 Web 自有 Rust 表面的首帧、调整尺寸、裁剪/叠层、焦点和销毁；缺失表面显示错误，不显示假就绪。
- [ ] Run `cargo test --workspace --locked`、`nsh check`、各 target package 和 surface 测试；Expected: 所有可执行环境真实通过；无环境者保持未验收，记录可复现 CI/设备步骤。
- [ ] 若 Web 网络必须新增受控代理，或 NSH 下限提高造成既有兼容损失，先给出实测证据与具体影响再交用户决定；其余实施自主推进。提交本任务文件。

### Task 3: PCB 二维与原理图全部 Rust 渲染

**Files:** Create `apps/nsh/src/viewer/{mod.rs,viewport.rs,drawing.rs,cache.rs,two_dimensional.rs}`、`apps/nsh/tests/{viewport.rs,drawing.rs,viewer.rs}`；reuse core scene/picking；move font/license assets without duplication after acceptance。

**Interfaces:** Consumes Task 2 scene ownership and surface; produces shared Rust viewer for PCB/schematic, cached parsed paths, bounded overview/detail rasters, retained camera/layer/selection state。

- [ ] 移植现有 viewport/raster/pictures 的独立手算断言和自制夹具；增加铜层 nonzero 重叠、孔洞、中文、旋转/镜像、图片与失败缓存保留测试。Run `cargo test -p oshwhub-nsh --test viewer`；Expected: 迁移行为缺失而失败。
- [ ] 先实现 Rust NSH Canvas 的正确显示与 retained transform，测其平台回放；仅在具体基准不满足时改为既定 NSH GPU 表面，保留同样的二维契约。
- [ ] 实现原子切层、180ms 空闲细化、有界预览/细节、单/双指/滚轮/键盘/无障碍操作、拾取与属性；面板不缩放/重建画布，过期响应不能改新场景。
- [ ] Run 新测试与原有 core tests、六视口 Web 像素/交互回归；Expected: 完整图元和状态保持正确，手势期间编译/路径解析次数 0，失败可恢复。
- [ ] 对应设备 Release 对照二维/原理图性能；保存原始样本与缺口，提交。

### Task 4: PCB 三维全部 Rust GPU 渲染

**Files:** Create `apps/nsh/src/viewer/{three_dimensional.rs,camera3d.rs,mesh.rs,gpu.rs,board.wgsl}`、`apps/nsh/tests/{camera3d.rs,gpu.rs}`；reuse Task 2 platform surfaces and core `solid`/`models`。

**Interfaces:** Consumes existing packed mesh/instances, scene ID and NSH surface; produces indexed/instanced renderer with small camera uniforms, progressive models and bounded lifecycle。

- [ ] 移植现有 camera/mesh/gl3d 业务断言；失败用例覆盖底面复合变换、索引/缓冲上限、迟到模型、隐藏暂停、初始化中途失败和释放。Run `cargo test -p oshwhub-nsh --test gpu`；Expected: 尚未实现而失败。
- [ ] 实现索引与实例绘制、与现有一致的深度/光照/颜色、每 mesh 一次上传、视图改变时按需绘制、模型并发/重试及统计；不增加每帧 CPU 全板投影。
- [ ] 各平台实际运行包含板厚/孔洞/铜/丝印/器件的场景；检查 2D/3D 切换状态与 20 次重复打开后的资源释放。
- [ ] Run 单测、Web 六视口/原生交互和 Release 对照；Expected: 视觉/行为正确，所有资源有界，手势 mesh 上传 0，性能不回退。保存证据并提交。

### Task 5: 全部业务页面、导航与持久化

**Files:** Create `apps/nsh/src/{model.rs,preferences.rs,pages/discover.rs,pages/project.rs,pages/bom.rs,pages/settings.rs,pages/about.rs,pages/lab.rs}` and shared controls; tests `model.rs`、`pages.rs`、`preferences.rs`。

**Interfaces:** Consumes Tasks 1/2 shell/services and Tasks 3/4 viewers; produces all original public workflows with owned Rust signals, virtualized collections and original theme preference values。

- [ ] 将 FeedController、主题解析、BOM 筛选、bridge 过期请求/错误、正文/图片、导航 E2E 的行为断言移植为 Rust/UI 测试。Run tests；Expected: 对应未迁移行为失败。
- [ ] 直接调用现有强类型 API，实现搜索防抖/提交/清除、排序/刷新/分页、去重、滚动恢复、详情/安全正文/按需图片/缩略回退、响应式 BOM 和过滤。
- [ ] 使用 NSH 虚拟列表、系统导航/返回和基础控件；补全加载/无结果/418/离线/重试、软键盘、短横屏、大字体、减少动态效果和隐藏区域焦点隔离。
- [ ] 幂等迁移主题/外观与应用身份；损坏/未知值回退默认，保存失败可见且不覆盖旧值。验证 NSH namespace 与旧 Android/iOS/Electron/Web 存储的真实位置。
- [ ] Run Rust/UI/Web 六视口回归、VoiceOver/TalkBack 和 Release 列表/BOM/启动性能对照；Expected: 全部工作流保持、异步交错不会回写错误页面。提交。

### Task 6: 完整性能门槛、产物与最终切换

**Files:** Add repeatable baseline/candidate capture and comparison under `scripts/` and `tests/`；modify `package.json`、workspace/build configs、`.github/workflows/ci.yml`、README/docs；remove old UI/runtime only after acceptance。

**Interfaces:** Consumes all prior tasks; produces per-platform same-input Release evidence, NSH entrypoints/artifacts, and a dependency tree free of old UI runtimes。

- [ ] 比较器失败测试覆盖缺少数据、错误设备/GPU/视口/样本 hash、样本不足、空值/NaN、部分指标回退和噪声内无结论；Expected: 任何不可信或回退对照均不能通过。
- [ ] 收集每组 10 次冷启动、5×60 秒交互、20 次生命周期以及 CPU/RSS/GPU 指标；小夹具/真实大板/合成压力分别归档；真实 GPU 与 SwiftShader 明确区分。
- [ ] 对所有功能与平台要求逐项审计，修复每项回退；Expected: 设计每个指标均有对应范围的可靠证据，未测项不得标完成。
- [ ] 更新各平台 NSH 构建、签名/安装说明和 CI 产物，验证正式包真实注册表面、字体/图片齐全与无 QuickJS；之后才切入口并删除 RN/Electron/Expo/Skia 和过时桥接/脚本。
- [ ] Run 全量 fmt/clippy/test、Web/原生回归、六平台 package 与最终同口径对照；Expected: 全部通过。独立整分支审阅并修复重要问题，最终提交；按用户授权决定远端交付，不能自动部署。
