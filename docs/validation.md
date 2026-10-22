# 首轮验收记录

日期：2026-10-23（Asia/Shanghai）。范围为公开项目浏览、搜索、详情、BOM 和专业版二维 PCB 查看。账号、写入、PCB 编辑和 3D 不在本轮范围。

## 离线检查

| 检查 | 结果与边界 |
| --- | --- |
| `pnpm check` | Biome、TypeScript、24 项 Vitest、7 项构建脚本测试、Rust fmt/clippy/test 通过；Rust 核心 49 项、开发服务 1 项、FFI 1 项，2 项测试默认忽略 |
| `prek run --all-files` | 全部 5 个 hook 通过；已安装本地提交 hook |
| `pnpm test:e2e` | 30/30 通过，最后一次 33.5 秒；使用自制接口响应，实际加载 CanvasKit WASM 和完整中文字体 |
| 开发服务超限回归 | 新增真实 TCP HTTP 测试通过：普通非法命令、恰好 1 MiB 成功、超过 1 MiB 拒绝且保留错误码和独立 UUID |
| CodeGraph | 已按用户授权初始化，本地索引不提交 |

E2E 尺寸：320×720、390×844、768×1024、1024×768、1440×900、844×390。覆盖搜索/排序/分页、详情正文、BOM、原页重试、滚动位置恢复、画布像素、中文、缩放按钮/键盘/滚轮、单指拖动、双指缩放、图层隐藏、可见层拾取，以及隐藏列表不再自动分页。图层控件默认可见层优先且切换后不跳位；新增生命周期回归覆盖 StrictMode、连续三次 Fast Refresh、卸载、实际像素及强制 GC 后的 Picture finalizer。新增回归还覆盖 Enter 的当前原生文本、搜索按钮同步草稿、正文真实 PNG 的解码/宽高比和空料号 BOM。无页面横向溢出、未处理的页面错误或非预期 console.error。浏览器采用 SwiftShader；这些结果不等于原生 GPU 和真机验收。

## 在线数据与真实 PCB

真实列表、正式搜索接口、详情及 BOM 的在线冒烟已执行。官网可能返回 418 或限流，错误码与 requestId 会显示在可重试页面。

PCB 样板工程 `90602f2f840d4245a97479e90a31d17b`，板子 `d4c95e111c8e5672`。最后一次 Debug 核心加载结果：

- 23 个历史、65 个分片；下载 9,906,972 字节，重放 282,807 条记录。
- 266 个文档，239/239 封装引用解析成功；7,569 个场景对象、1,564 个批次。
- 总加载 8.065 秒，解析 3.420 秒，场景编译 207 毫秒；包含网络耗时，不能当作跨设备帧率基准。
- 预期警告：2 项 DIMENSION 样式、7 项失效铺铜、15 项非零多边形焊盘扩展、6 项无嵌入字体回退；无缺失封装。

共享 UI 已通过实际在线流程复核：搜索到上述工程，打开详情、板子/BOM 和真实 PCB，完成缩放、拾取、隐藏铜层和适应窗口。修复后真实正文全部图片解码成功，浏览器控制台无错误。

独立审阅修复并回归：client 字段内存放大、拼接 gzip 后续删除记录丢失、已删除 DEVICE 仍显示、隐藏层组件 bbox 误拾取、bbox 遮盖真实图元拾取、旋转文字边界和多边形零扩展 mask/paste。格式覆盖及剩余警告见 [renderer.md](renderer.md)。

样板为 CC BY-NC-SA 4.0。其解密历史、板数据和预览素材未提交；仓库测试夹具及交互示例为自制内容。

## 原生构建与运行

| 平台 | 本机证据 | 仍需验证 |
| --- | --- | --- |
| Android | arm64-v8a/x86_64 Rust 库、Debug 与 Release APK 构建成功；最终 Release 已安装 emulator-5554，无需 Metro。真实搜索、详情、板子/BOM、7,569 图元 PCB、缩放、平移和拾取成功 | 真机 GPU/多指手势、旋转及完整 IME 时序 |
| iPhone | iOS 27.0 / iPhone 18 Pro Max 模拟器 Release 构建、安装和启动成功，真实列表可见 | 真机、签名及完整原生手势验收 |
| iPad | iOS 27.0 / iPad Pro 11-inch (M5) 模拟器安装相同 Release，真实三列列表和安全区域可见 | 分屏、大字体及真机手势验收 |
| macOS | Apple Silicon Release `.app` 与 `.dmg` 构建成功，原生 Rust 已成功获取真实列表 | 正式包动态样式修复后的界面及外链验收；当前系统锁屏妨碍 UI 自动化 |
| Windows / Linux | 最终代码 `7abf60d` 的 GitHub Actions Tauri 构建成功并上传安装包 | Windows/Linux 原生运行、字体及 GPU 验收 |

Android 软件 GPU 模拟器的内存优化版 Release 真实板载入 6,917 ms、解码 603 ms、场景编译 90 ms；此前一次为 7,412/1,009/98 ms。网络耗时会波动。系统 DNS 故障已通过显式 DNS 和 SwiftShader 启动模拟器恢复；数据未清除。此结果不能代替真机性能基准。

最终 `7abf60d` Release 再次验证同一真板：载入 7,945 ms、解码 641 ms、场景 96 ms。常用图层出现在控件前部；隐藏/恢复 Top Layer、125% 缩放、平移、适应窗口及 R24 属性拾取成功。iPhone/iPad 最终包也已终止旧进程后重新安装、启动并确认真实列表与安全区域。

### PCB 内存复测

同一真实工程、同一 Android 模拟器，分别从新进程首页进入 PCB；使用 `adb shell dumpsys meminfo com.gouzil.oshwhub` 读取稳定后 PSS，单位 KiB。不是加载期间的峰值采样。

| 场景 | 优化前 PSS / Native Heap | 优化后 PSS / Native Heap |
| --- | --- | --- |
| 首页 | 124,850 / 49,860 | 132,947 / 49,980 |
| PCB 完整显示 | 869,925 / 731,584 | 294,209 / 189,848 |
| 返回详情 | 852,353 / 732,816 | 299,803 / 191,856 |

查看期间 PSS 减少约 66%。优化后仍能拾取 R24，返回完整 componentId、recordId 和 ticket。该对照只改变 Rust 工程生命周期与 header 存储，使用相同的原生 JS 查看器；后续图层 UI 调整另验。

本机 Release 核心的同输入计数探针：重放峰值活跃分配从 399.4 MiB 降到 239.5 MiB；load 返回后的活跃分配从 389.4 MiB 降到 16.8 MiB；释放返回的绘制数据后，两个以内的拾取场景缓存此次占 13.8 MiB。独立 smoke 的进程峰值 RSS 从 515.0 MiB 降到 333.3 MiB。排除随机场景 ID、requestId 和耗时后，真实场景输出逐字段一致。

完整记录在编译后释放，未知 header 字段和 tombstone 在重放期间保留；未来编辑入口必须重新加载源工程。当前仍非流式解析，allocator 可能保留已释放页，活跃分配与 RSS/PSS 不应混用。

最终包同进程连续三次打开真板均成功，载入分别为 7,945、7,471、7,790 ms。第三次刚加载后总 PSS 达 664,028 KiB，约 98.6 秒后无 UI 操作或强制 GC，自行降至 353,264 KiB，与第二次的 353,741 KiB 基本持平；同期 Native Heap Alloc 仅从 218,187 降至 217,400 KiB。大幅波动主体为已释放页面的驻留时点。该短样本支持保留当前两个拾取场景的缓存上限，不能代替长期真机泄漏/峰值测试；后续基准需固定 idle 测量时间并同时记录 Alloc、Free 和 PSS。

本地构建产物（均不提交 Git）：

```text
apps/mobile/android/app/build/outputs/apk/release/app-release.apk
apps/mobile/ios/build/Build/Products/Release-iphonesimulator/OSHWHub.app
target/release/bundle/macos/OSHWHub.app
target/release/bundle/dmg/OSHWHub_0.1.0_aarch64.dmg
```

Android Release 当前使用开发签名；iOS 为未签名模拟器构建，桌面未做商店签名/公证。发布商店和真机分发需要各平台正式签名。

最终代码版本为 `7abf60d38568a4589228a11f8a02c45750fb0bff`。本机最后重建的产物 SHA-256：

```text
APK  37720fe6229d90e318e654d9d632413b2ff93d64a8aed322b0ffebc67d6a0713
DMG  22c43b6cbfcda763590f12174b489795d47782842dcf0f4736e6b69cd900b223
```

## 交付与后续

阶段提交已推送至 `main`：`b6afb5b` 设计、`9ecbdb7` 独立渲染方案、`c374034` Rust 核心、`c81d117` 共享客户端、`3dcf55c` CI/质量、`b5b7e0a` 实际 UI 回归与 Android SDK 安装修复、`fa4f534` PCB 内存优化、`51dd1af` Android 共享源码增量打包、`7abf60d` Picture 生命周期与图层操作。

首轮 [CI #39830220901](https://github.com/gouzil/oshwhub_app/actions/runs/39830220901) 的 quality、iOS 和三个桌面平台均成功；Android 在过时 setup-android v3 的 tools 包安装阶段失败。已升级 v4 并显式只安装 platform-tools。

第二轮 [CI #39833066803](https://github.com/gouzil/oshwhub_app/actions/runs/39833066803) 的 quality、Windows、Linux 成功。macOS/iOS 因 hosted runner 容量不足未启动并取消；Android 在原生依赖编译中收到 runner shutdown 信号而中断，日志没有源码编译错误证据。本机对应 Android/iOS/macOS Release 构建通过。

最终代码 `7abf60d38568a4589228a11f8a02c45750fb0bff` 的 [CI #39839629388](https://github.com/gouzil/oshwhub_app/actions/runs/39839629388)，截至 2026-10-23 03:46（Asia/Shanghai）：quality、iOS 和 Windows/macOS/Linux 三个桌面构建全部成功，平台产物已上传。quality 中统一检查、prek、Web 构建及 30 项 E2E 通过（CI E2E 3.1 分钟）。Android 的 SDK、Rust 双架构和原生工程生成已成功，Gradle Release 仍在编译，尚未取得该 job 的最终结论。本节文档提交跳过 CI；构建与测试证据对应上述代码版本。

本轮不宣称覆盖所有专业版文档类型或全部设备。重点后续项为真机性能采样、复杂板的首帧/帧时间/内存基准，以及文档中明确提示的格式兼容扩展；账号功能按用户确认另行迭代。
