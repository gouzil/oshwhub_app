# 首轮验收记录

日期：2026-10-23（Asia/Shanghai）。范围为公开项目浏览、搜索、详情、BOM 和专业版二维 PCB 查看。账号、写入、PCB 编辑和 3D 不在本轮范围。

## 离线检查

| 检查 | 结果与边界 |
| --- | --- |
| `pnpm check` | Biome、TypeScript、24 项 Vitest、4 项构建脚本测试、Rust fmt/clippy/test 通过；Rust 核心 47 项、FFI 1 项，2 项在线测试默认忽略 |
| `prek run --all-files` | 全部 5 个 hook 通过；已安装本地提交 hook |
| `pnpm test:e2e` | 18/18 通过，最后一次 42.5 秒；使用自制接口响应，实际加载 CanvasKit WASM 和完整中文字体 |
| 开发服务超限回归 | 新增真实 TCP HTTP 测试通过：普通非法命令、恰好 1 MiB 成功、超过 1 MiB 拒绝且保留错误码和独立 UUID |
| CodeGraph | 已按用户授权初始化，本地索引不提交 |

E2E 尺寸：320×720、390×844、768×1024、1024×768、1440×900、844×390。覆盖搜索/排序/分页、详情正文、BOM、原页重试、滚动位置恢复、画布像素、中文、缩放按钮/键盘/滚轮、单指拖动、双指缩放、图层隐藏、可见层拾取，以及隐藏列表不再自动分页。无页面横向溢出、未处理的页面错误或非预期 console.error。浏览器采用 SwiftShader；这些结果不等于原生 GPU 和真机验收。

## 在线数据与真实 PCB

真实列表、正式搜索接口、详情及 BOM 的在线冒烟已执行。官网可能返回 418 或限流，错误码与 requestId 会显示在可重试页面。

PCB 样板工程 `90602f2f840d4245a97479e90a31d17b`，板子 `d4c95e111c8e5672`。最后一次 Debug 核心加载结果：

- 23 个历史、65 个分片；下载 9,906,972 字节，重放 282,807 条记录。
- 266 个文档，239/239 封装引用解析成功；7,569 个场景对象、1,564 个批次。
- 总加载 8.065 秒，解析 3.420 秒，场景编译 207 毫秒；包含网络耗时，不能当作跨设备帧率基准。
- 预期警告：2 项 DIMENSION 样式、7 项失效铺铜、15 项非零多边形焊盘扩展、6 项无嵌入字体回退；无缺失封装。

独立审阅修复并回归：client 字段内存放大、拼接 gzip 后续删除记录丢失、已删除 DEVICE 仍显示、隐藏层组件 bbox 误拾取、bbox 遮盖真实图元拾取、旋转文字边界和多边形零扩展 mask/paste。格式覆盖及剩余警告见 [renderer.md](renderer.md)。

样板为 CC BY-NC-SA 4.0。其解密历史、板数据和预览素材未提交；仓库测试夹具及交互示例为自制内容。

## 原生构建与运行

| 平台 | 本机证据 | 仍需验证 |
| --- | --- | --- |
| Android | arm64-v8a/x86_64 Rust 库、Debug 与 Release APK 构建成功；Debug 已安装 emulator-5554，真实错误页与 requestId 可见 | 模拟器发生 DNS 故障及系统 ANR，正在进行 Release 安装、联网与查看器验收 |
| iPhone | iOS 27.0 / iPhone 18 Pro Max 模拟器 Release 构建、安装和启动成功，真实列表可见 | 真机、签名及完整原生手势验收 |
| iPad | iOS 27.0 / iPad Pro 11-inch (M5) 模拟器安装相同 Release，真实三列列表和安全区域可见 | 分屏、大字体及真机手势验收 |
| macOS | Apple Silicon Release `.app` 与 `.dmg` 构建成功，原生 Rust 已成功获取真实列表 | 正式包动态样式修复后的界面及外链验收；当前系统锁屏妨碍 UI 自动化 |
| Windows / Linux | 已配置各自 Tauri 构建 CI | 尚未取得本轮实际 CI 构建结果和本机运行证据 |

本地构建产物（均不提交 Git）：

```text
apps/mobile/android/app/build/outputs/apk/release/app-release.apk
apps/mobile/ios/build/Build/Products/Release-iphonesimulator/OSHWHub.app
target/release/bundle/macos/OSHWHub.app
target/release/bundle/dmg/OSHWHub_0.1.0_aarch64.dmg
```

Android Release 当前使用开发签名；iOS 为未签名模拟器构建，桌面未做商店签名/公证。发布商店和真机分发需要各平台正式签名。

## 交付与后续

阶段提交已包含独立 Rust 核心和共享 React Native / Skia 客户端。CI、原生运行复核与最终远端 SHA 将在检查完成后补充到本记录。

本轮不宣称覆盖所有专业版文档类型或全部设备。重点后续项为真机性能采样、复杂板的首帧/帧时间/内存基准，以及文档中明确提示的格式兼容扩展；账号功能按用户确认另行迭代。
