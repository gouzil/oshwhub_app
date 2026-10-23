# 首轮验收记录

日期：2026-10-23（Asia/Shanghai）。范围为公开项目浏览、搜索、详情、BOM 和专业版二维 PCB 查看。账号、写入、PCB 编辑和 3D 不在本轮范围。

## 离线检查

| 检查 | 结果与边界 |
| --- | --- |
| `pnpm check` | Biome、TypeScript、29 项 Vitest、12 项构建脚本测试、Rust fmt/clippy/test 通过；Rust 核心 49 项、开发服务 1 项、FFI 1 项，2 项测试默认忽略 |
| `prek run --all-files` | 全部 5 个 hook 通过；已安装本地提交 hook |
| `pnpm test:e2e` | 72/72 通过，最后一次 52.7 秒；使用自制接口响应，实际加载 CanvasKit WASM 和完整中文字体 |
| WebKit 补测 | 玻璃导航阶段 1440×900 下 6 项通过、1 项 CDP 透明度模拟显式跳过，4.6 秒；覆盖导航与全部加载回归。上一阶段另有示例板中文/实际像素、铜层隐藏与恢复证据；未在 WebKit 运行 Chromium CDP 手势和生命周期用例 |
| 开发服务超限回归 | 新增真实 TCP HTTP 测试通过：普通非法命令、恰好 1 MiB 成功、超过 1 MiB 拒绝且保留错误码和独立 UUID |
| CodeGraph | 已按用户授权初始化，本地索引不提交 |

E2E 尺寸：320×720、390×844、768×1024、1024×768、1440×900、844×390。覆盖搜索/排序/分页、详情正文、BOM、原页重试、滚动位置恢复、画布像素、中文、缩放按钮/键盘/滚轮、单指拖动、双指缩放、图层隐藏、可见层拾取，以及隐藏列表不再自动分页。图层控件默认可见层优先且切换后不跳位；新增生命周期回归覆盖 StrictMode、连续三次 Fast Refresh、卸载、实际像素及强制 GC 后的 Picture finalizer。新增回归还覆盖 Enter 的当前原生文本、搜索按钮同步草稿、正文真实 PNG 的解码/宽高比和空料号 BOM。无页面横向溢出、未处理的页面错误或非预期 console.error。浏览器采用 SwiftShader；这些结果不等于原生 GPU 和真机验收。

紧凑窗口补测先复现失败再修复：3 个小画布用例验证板子保持在视野内且不因固定留白缩成几个像素；E2E 验证横屏不显示遮挡画布的操作提示，并在窗口进一步缩至 240px 高时确认网络属性与关闭按钮完整可见。

本轮新增加载回归覆盖首页/详情的脉冲骨架、减少动态效果、长正文仅渲染部分条目、远处图片不发请求、进入可视范围后占位到真实图片的切换、缩略损坏后回退原图，以及正文/封面独立重试且不重新获取工程。封面重试和长海报自然比例均先验证失败，再修复并在六个尺寸通过。只读审阅提出的骨架原生 `accessible` 标记也已补齐；iPad 原生无障碍树实际返回“正在打开项目 / busy”，尚未现场操作 VoiceOver/TalkBack。

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
| Android | arm64-v8a/x86_64 Rust 库、Debug 与 Release APK 构建成功；最终 Release 已安装 emulator-5554，无需 Metro。真实搜索、详情、板子/BOM、7,569 图元 PCB、缩放、平移和拾取成功；已补测横屏及两倍系统字体的查看器交互 | 真机 GPU/多指手势及完整 IME 时序 |
| iPhone | iOS 27.0 / iPhone 18 Pro Max 模拟器 Release 构建、安装和启动成功；真实列表、搜索、详情骨架、正文后段照片，以及 Safari 原图外链通过 | 真机、签名及完整原生 PCB 手势验收 |
| iPad | iOS 27.0 / iPad Pro 11-inch (M5) 模拟器安装相同 Release；真实三列列表、封面、详情骨架及其原生 busy 提示可见，正文图片独立重试成功；最终包长海报按正文宽度显示，首段、中段和末段滚动可读 | 分屏、大字体及真机手势验收 |
| macOS | Apple Silicon Release `.app` 与 `.dmg` 构建成功；真实列表、搜索、详情、正文图片、BOM、7,569 图元 PCB、125% 缩放和适应窗口通过；正文原图由默认 Edge 打开；窗口置前后铜层隐藏/恢复的实际像素正确 | 后台恢复与首帧长期稳定性、复杂板帧时间基准 |
| Windows / Linux | 代码 `d77063b` 的 GitHub Actions Tauri 构建均成功并上传安装包；本轮加载改动等待新 CI | Windows/Linux 原生运行、字体及 GPU 验收 |

Android 软件 GPU 模拟器的内存优化版 Release 真实板载入 6,917 ms、解码 603 ms、场景编译 90 ms；此前一次为 7,412/1,009/98 ms。网络耗时会波动。系统 DNS 故障已通过显式 DNS 和 SwiftShader 启动模拟器恢复；数据未清除。此结果不能代替真机性能基准。

此前 `7abf60d` Release 再次验证同一真板：载入 7,945 ms、解码 641 ms、场景 96 ms。常用图层出现在控件前部；隐藏/恢复 Top Layer、125% 缩放、平移、适应窗口及 R24 属性拾取成功。

`6f7bbf8` Release 补测：Android 普通字号横屏显示真实 7,569 图元 PCB，125% 缩放和适应窗口通过，载入 7,640 ms、解码 724 ms、场景编译 100 ms。两倍系统字体叠加横屏时，复现并修复了固定留白缩小板子、操作提示遮挡板子、属性正文受画布高度限制的问题。新包本地示例的 125% 缩放、适应窗口、ARC 属性拾取、完整 GND/recordId/ticket 和关闭面板均通过；同一 194px 高画布下，网络属性行由旧包裁剩的 6px 恢复到完整 94px。测试后已恢复默认字号和自动旋转。iPhone/iPad 新包已终止旧进程后重新安装、启动，真实列表与安全区域正常。Android/iOS/macOS Release 均重新构建成功。

### 本轮加载优化

本轮原生包对应代码 `1ac8b68286b12a9142fe65b9d59d788ff804c635`，最后一次修复长海报后 Android/iOS/macOS Release 均重新构建成功。Android 本轮已验证真实搜索、详情封面与正文滚动入口，最终包重新安装后冷启动返回 `Status: ok`，未补测首张正文图片解码后的画面；iPhone 本轮已验证正文末段图片、附件区域和 Safari 原图外链，最终包重新安装启动；iPad 最终包已观察三列真实列表、封面缩略图与安全区域，并验证 800×10,000 海报按正文宽度显示，可滚到中段应用场景和末段发货清单、原图链接与 BOM 按钮。Device Hub 主窗口的拖动未生效，切换独立设备窗口后手势成功；该工具现象未通过修改应用代码规避。macOS 已退出旧进程并启动最终包，前台确认详情骨架、真实封面和长海报自然比例。上述列表/正文证据与此前 PCB 横屏、大字体证据分别记录，不冒称本轮重测了全部原生手势。

macOS 两次真板载入分别为 9,769/2,692/525 ms 和 7,435/435/64 ms（总载入/解码/场景编译）。后台自动化曾观察到控件已更新但画布为空或保留上一帧；通过窗口的 `Raise` 操作置前后，首幅板图与铜层隐藏立即可见，恢复图层也正常。未修改渲染器来规避后台节流，也未将此短样本当作所有前后台切换的稳定性保证。后续原生自动化必须明确置前窗口再判断画布像素。

正文接口仍一次返回完整块数组；本轮分块的是 FlatList 渲染和图片请求。单图 CDN 样本 `c097ba671a9046ee9d04d40b54b1ee29.png`：原 URL 270,382 bytes / 2.38 秒，960px q80 WebP 53,258 bytes / 1.45 秒，体积减少约 80.3%。这是一次真实 HTTP 样本，不是全站平均或真机性能基准。iPad 的泰山派3M工程把整个介绍放在一张 800×10,000 长海报中；实测原图 637,698 bytes，1920px 处理请求未放大原图，仍为 800×10,000、623,510 bytes，缩略处理不能保证每张图都有相同收益。该图首次请求失败后单图重试成功；实测还发现统一 520px 高度上限会使海报不可读，已取消正文高度上限并补充自然比例回归。单张海报仍是一次图片下载，没有切成多个下载分片。设计、失败回退和签名 URL 边界见 [design.md](design.md)。

### 本轮玻璃导航

按用户确认同时调整底部导航和桌面侧栏。iOS 新增 `expo-glass-effect` 58.0.4，重新安装 pods 后构建 Release，构建日志确认 `ExpoGlassEffect` 链接成功。Android、iOS 模拟器和 macOS Release 均构建成功；不是仅更新 Metro 中的 JS。

本机统一检查、5 个 prek hook、72 项 Chromium E2E 全部通过。导航新增回归先复现缺少材质，再验证六个视口的圆角悬浮/侧栏、滤镜、选中状态、44px 触控目标和页面末尾不被遮挡；Chromium CDP 模拟“降低透明度”后实色回退及切页通过。初次全量回归有三项短横屏用例失败：滚到最后一张卡片的最近边缘不足以触发分页，直接跳到页尾则越过了尚未进入视口的正文图片。测试已改为滚到最后卡片起点、先滚入正文图片区域，保留分页、缩略回退、重试和解码比例断言；最终全量 72/72 通过。WebKit 中导航及五项加载用例通过，透明度模拟因依赖 CDP 明确跳过。

原生/正式包现场证据：iPhone 18 Pro Max / iOS 27.0 的底栏可见玻璃透射与圆角亮边，切换到 PCB 实验室后绿色胶囊状态正确；打开本地示例板后底栏收起、画布和安全区域正常。iPad Pro 11-inch (M5) / iPadOS 27.0 横屏显示圆角原生玻璃侧栏和真实四列信息流。macOS 退出旧进程、启动新 Release 并置前后，玻璃侧栏、导航切页与选中态正常。Android 最终 APK 安装到 emulator-5554，冷启动 `Status: ok`；实色胶囊回退正常，点击 PCB 实验室后页面和原生 selected 状态正确。独立只读代码审阅未发现阻塞问题。

本轮未现场切换 iOS 的降低透明度或极大字体，也未操作 VoiceOver/TalkBack；这些设置路径目前有代码检查和浏览器对应行为证据，不能当作原生手动验收。旧 iOS 系统、真机 GPU/功耗和 Windows/Linux 原生外观仍需分别验证。桌面是 CSS 玻璃近似，Android 当前使用实色回退，不宣称具有 UIKit 的原生折射效果。

本轮产物 SHA-256（路径与下文相同）：

```text
APK  3b4e12d0950925a98b2325b1f235b17d92342bd7a4ba427424ac5f4acb463369
DMG  5c45991bd44820dbb98fd966a2feab31091f657e771b36d4f827744ad4d386dd
iOS main.jsbundle  6a1aae0fa6f20b3d33f08a7cf646ef02e24db6711ff3a61419f93dbfdbf58365
```

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

`7abf60d` 包同进程连续三次打开真板均成功，载入分别为 7,945、7,471、7,790 ms。第三次刚加载后总 PSS 达 664,028 KiB，约 98.6 秒后无 UI 操作或强制 GC，自行降至 353,264 KiB，与第二次的 353,741 KiB 基本持平；同期 Native Heap Alloc 仅从 218,187 降至 217,400 KiB。大幅波动主体为已释放页面的驻留时点。该短样本支持保留当前两个拾取场景的缓存上限，不能代替长期真机泄漏/峰值测试；后续基准需固定 idle 测量时间并同时记录 Alloc、Free 和 PSS。

本地构建产物（均不提交 Git）：

```text
apps/mobile/android/app/build/outputs/apk/release/app-release.apk
apps/mobile/ios/build/Build/Products/Release-iphonesimulator/OSHWHub.app
target/release/bundle/macos/OSHWHub.app
target/release/bundle/dmg/OSHWHub_0.1.0_aarch64.dmg
```

Android Release 当前使用开发签名；iOS 为未签名模拟器构建，桌面未做商店签名/公证。发布商店和真机分发需要各平台正式签名。

以下为代码 `1ac8b68` 本轮加载优化最终构建产物的 SHA-256：

```text
APK  71a04b7bfab384347a9196be8caf93af247fc463bd38ed8893710eea401325d9
DMG  04ead324d31ca8686bdddd46216fcd2d2a9f4478c5b4b543391e2321b93c8903
iOS main.jsbundle  5ac4625cbc539f70b23235c231fd24130c048291baa53f63e67b12e5a1fa71dd
```

## 交付与后续

阶段提交已推送至 `main`：`b6afb5b` 设计、`9ecbdb7` 独立渲染方案、`c374034` Rust 核心、`c81d117` 共享客户端、`3dcf55c` CI/质量、`b5b7e0a` 实际 UI 回归与 Android SDK 安装修复、`fa4f534` PCB 内存优化、`51dd1af` Android 共享源码增量打包、`7abf60d` Picture 生命周期与图层操作、`2d20ad0` 验收记录、`6f7bbf8` 紧凑画布与属性面板适配、`d77063b` 原生搜索和正文兼容、`1ac8b68` 骨架、正文按需图片与长海报修复。

首轮 [CI #39830220901](https://github.com/gouzil/oshwhub_app/actions/runs/39830220901) 的 quality、iOS 和三个桌面平台均成功；Android 在过时 setup-android v3 的 tools 包安装阶段失败。已升级 v4 并显式只安装 platform-tools。

第二轮 [CI #39833066803](https://github.com/gouzil/oshwhub_app/actions/runs/39833066803) 的 quality、Windows、Linux 成功。macOS/iOS 因 hosted runner 容量不足未启动并取消；Android 在原生依赖编译中收到 runner shutdown 信号而中断，日志没有源码编译错误证据。本机对应 Android/iOS/macOS Release 构建通过。

代码 `7abf60d38568a4589228a11f8a02c45750fb0bff` 的 [CI #39839629388](https://github.com/gouzil/oshwhub_app/actions/runs/39839629388)：quality、iOS 和 Windows/macOS/Linux 三个桌面构建全部成功，平台产物已上传。quality 中统一检查、prek、Web 构建及 30 项 E2E 通过（CI E2E 3.1 分钟）。Android 的 SDK、Rust 双架构和原生工程生成已成功，Gradle Release 未取得成功结论；新代码推送后已取消该过时运行，最终状态为 cancelled，不能当作源码编译失败。

代码 `6f7bbf8268589a93195e7aa13e0a823460bb6402` 的 [CI #39846343412](https://github.com/gouzil/oshwhub_app/actions/runs/39846343412) 已结束：quality、Windows、Linux 成功，Android 失败，macOS/iOS 取消；整轮为 failure。quality 日志确认 27 项 TS 测试、7 项构建脚本测试、51 项 Rust 测试及 30 项 E2E 通过（CI E2E 2.9 分钟）。

代码 `d77063b1eddcd15d36a630dd9a461acb098ca2b6` 的 [CI #39881811212](https://github.com/gouzil/oshwhub_app/actions/runs/39881811212) 已结束：quality、iOS、Windows、Linux、macOS 全部成功；Android 在 SDK manager 下载 NDK 27.1.12297006 时出现 `Error reading Zip content from a SeekableByteChannel`，尚未进入 Rust/Gradle 构建，整轮为 failure。该结果不能当作 Android 源码编译失败。

代码 `1ac8b68286b12a9142fe65b9d59d788ff804c635` 已推送并触发 [CI #39890604300](https://github.com/gouzil/oshwhub_app/actions/runs/39890604300)。2026-10-23 本轮记录时仍在执行；上面的 60 项 E2E、WebKit 6 项、统一检查、prek 及 Android/iOS/macOS Release 均为本机结果。文档提交跳过 CI，避免取消正在执行的代码构建。

系统已解锁，本轮 macOS 正式包界面与外链验证已继续完成；此前锁屏阻挡已解除。文档更新与源码构建证据按各自版本区分。

本轮不宣称覆盖所有专业版文档类型或全部设备。重点后续项为真机性能采样、复杂板的首帧/帧时间/内存基准，以及文档中明确提示的格式兼容扩展；账号功能按用户确认另行迭代。
