# OSHWHub 跨端客户端设计

日期：2026-10-23（Asia/Shanghai）。首轮范围已由用户确认：先完成公开浏览和 PCB 查看，再迭代账号功能。

## 目标与边界

在 Android、iPhone/iPad 和 Windows/macOS/Linux 桌面端复用 React Native 界面。支持公开工程信息流、分页、搜索、排序、工程详情、BOM、板子选择和二维 PCB 查看。能在 320px 手机、小屏横屏、平板分屏和宽屏窗口中伸缩，保留安全区域、可访问标签和键盘操作。平台支持、实际构建、模拟器验证和真机验证分别记录，不用“完美适配”代替验收。

参考 `/Users/gouzi/Downloads/app_zh_release.apk`（Bambu Handy 4.3.0）的搜索、分类标签、卡片信息流和底部导航布局；不复制其代码和素材。桌面端使用侧栏和可伸缩内容区；详情和查看器在窄屏以独立页面呈现，在宽屏利用额外空间。

本轮不实现账号登录、评论写入、云收藏同步、PCB 编辑、三维模型或制造导出。离线数据必须明确标注，不能用示例冒充实时工程。

## 技术选择

- React Native + Expo，使用开发构建接入本地 Rust 模块；不以 Expo Go 作为原生桥验收。
- React Native Web + Tauri 2 作为桌面外壳，保持页面和交互代码共享。桌面 HTML 是同一套 React Native 组件编译结果。
- Rust 2024 workspace：共享核心执行网络访问、参数校验、分页和字段归一化、正文安全转换、BOM 解析、EDA 历史下载/解密/合并与压缩归档。
- Android 通过 Expo Modules + JNI 调用 Rust；iOS 通过 Expo Modules + C ABI；桌面通过 Tauri command。协议为版本化 JSON，请求/响应有稳定的类型与结构化错误。
- 浏览器仅作开发预览，通过只绑定 loopback 的 Rust 开发服务调用相同核心；正式移动和桌面包不依赖该服务。
- PCB 使用自研二维渲染器：Rust 保留文档状态、转换几何、生成图层绘制批次和执行拾取，React Native Skia 绘制本地场景。移动端使用原生 Skia，桌面使用同组件的 CanvasKit 后端；不复用 `easyeda-viewer` 或闭源 `smt-gl-engine` 的实现。

对比方案：全部 JS 业务实现不满足 Rust 业务主体要求；已有二维预览器虽能减少首轮工作，但用户已明确要求自研。采用独立文档、场景和绘制三层，格式兼容与 GPU 绘制分别验证。此处的“smt-gl-engine”表示自研图形引擎方向，不表示已复刻专业版全部功能。

## 目录与依赖方向

```text
apps/mobile          Expo 入口、平台配置
apps/desktop         React Native Web + Tauri 外壳
packages/ui          共享界面、布局、加载/错误/空状态
packages/bridge      TypeScript 协议和平台适配
packages/pcb-viewer  自研 Skia 绘制、图片缓存和触摸适配
modules/oshwhub-core Expo 原生模块（Kotlin / Swift）
crates/core          Rust 业务、网络和 PCB 数据准备
crates/ffi           C ABI / JNI，仅桥接不重复业务
crates/dev-server    loopback 开发服务
docs                 架构、协议、开发、渲染、验收和已知边界
```

UI -> bridge -> Rust core；渲染器只消费 Rust 准备的场景，不直接访问官网接口。核心测试不依赖手机、桌面 WebView 或外网。网络测试独立运行并明确标记。

## 数据与错误处理

公开 API 使用固定站点，允许列表、搜索、详情和工程元数据查询。UUID、页码、关键词长度在 Rust 边界校验。网络请求设置连接/总超时、响应大小和重定向限制；不能把服务器返回的任意 URL 当作可下载目标。详情 HTML 转成安全的内容块，外链仅允许 http/https，不在应用内执行工程正文中的脚本。

错误协议包含 code、用户可理解的 message 和 requestId。界面区分加载、无结果、服务拒绝、离线和损坏工程，提供重试。切换搜索/详情后过期响应不得覆盖当前结果。页码只在成功后推进，重试不得重复卡片。

账号与凭据不进入首轮。日志只记录请求类型、状态、耗时、数量和关联 ID，不记录正文、Cookie、Authorization、解密密钥和完整个人信息。原生核心使用 `tracing`，界面记录渲染/桥接故障；调试日志与运行说明一起提供。

## PCB 文档与性能

`proImages` 只提供板子、PCB ID、缩略图和 BOM，不能替代真实 PCB。完整链路为项目元数据 -> 默认分支 -> 历史页 -> 下载载荷 -> AES-GCM 验证解密 -> gzip 解压 -> 按历史顺序和文档/记录 ID 合并 -> Rust 场景批次 -> Skia 本地绘制。

保留文档边界、图元 ID、ticket 和删除标记。封装/字体/图片等依赖必须明确处理；历史链不完整、解密失败、未知结构或缺少必要依赖时报告错误/警告，不能静默展示一张“完整”板。格式支持按真实夹具和实际渲染结果记录。

查看器独立于 React 业务组件树，不能每条走线创建 React 节点。按图层/画笔批量构建和缓存 SkPicture；工程只在加载时跨桥，平移/缩放/图层切换留在绘制层，不在手势期间传整板 JSON。固定输入/解压大小和历史数量上限，拒绝压缩炸弹。记录下载、解码、合并、场景构建与首次绘制耗时，给出实际板子和设备信息，避免空泛性能承诺。

触控支持单指平移、双指中心缩放、缩放按钮、适应窗口；桌面支持鼠标滚轮、拖动、键盘和可访问工具栏。查看器卸载时释放事件监听、缓存和画布资源。

## 验收与持续集成

Rust 单测覆盖 API 归一化、URL/参数校验、错误、历史顺序、重复/删除/损坏记录、解密和解压上限。TypeScript 测试覆盖桥协议、异步过期响应、布局边界和查看器触摸计算。E2E 在手机、平板和桌面尺寸执行浏览、详情和 PCB 交互，检查溢出与运行时错误。

提交前运行 Rust fmt/clippy/test、TypeScript check、格式/静态检查和相关测试。使用 prek 固定本地检查入口。CI 安装锁定工具链和依赖，运行相同检查，并分别验证 Android、iOS 与桌面构建；上传构建产物和失败诊断。在线站点测试单独列出，不能以外部不可用掩盖离线单测失败。
