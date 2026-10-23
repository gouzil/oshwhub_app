# 验收记录

首轮日期：2026-10-23（Asia/Shanghai）。当时范围为公开项目浏览、搜索、详情、BOM 和专业版二维 PCB 查看。后续三维阶段记录如下；账号、写入和 PCB 编辑仍待后续迭代。

## 2026-10-24 三维查看阶段

PCB 工作区新增 2D / 3D 切换，二维相机、图层和选中状态保留。三维支持单指旋转、双指平移/缩放、滚轮与键盘、顶底视角、适应窗口、器件显隐和模型重试。Rust 编译板体及模型装配，独立 WebGL / Expo GL 只在输入或资源变化时绘制；重复器件共享 GPU 缓冲，模型请求最多三个并发，GPU 缓冲限制为 64 MiB。设计与格式边界见 [3d.md](3d.md)。

| 检查 | 本轮证据与边界 |
| --- | --- |
| `pnpm check` | Biome、TypeScript、42 项 Vitest、12 项构建脚本测试、Rust fmt/clippy 通过；Rust 核心 47 项、API 契约 19 项、开发服务与 FFI 各 1 项通过，2 项测试按原设计忽略 |
| 六视口三维回归 | 30/30 通过，覆盖旋转时不重建板体、双指平移缩放、窗口变化、二维状态保留、隐藏区键盘隔离、模型渐进加载/重试、离开后过期响应、GPU 资源回收及 WebGL context loss 恢复 |
| 完整 E2E | 最终无诊断代码运行 174/174 通过（2.5 分钟）。修复前在 desktop-1024 / wide-1440 重复捕获首页极速输入丢字；修复后两个桌面尺寸合计连续 40 次通过（1.2 分钟），再通过六视口完整回归 |
| `prek run --all-files` | 暂存全部新增文件与最终搜索修复后，5 个 hook 全部通过 |
| 原生构建 | iOS 三架构 Rust / XCFramework、Android arm64/x86_64 Rust、iOS Simulator Release、Android Release、macOS `.app` 与 DMG 均构建成功。macOS DMG 首次 Finder 卷定位失败，使用与 CI 一致的 `CI=true` 重建后成功 |

首页回归的失败 trace 显示：点击搜索前，输入框已经缺少末尾字符；原实现每键更新整块列表并回写受控文本。改为输入控件持有编辑草稿，JS ref 同步当前内容，只在显式清除时调用原生 `clear()`，避免用迟到的渲染覆盖输入。Enter 仍读取当前原生文本，清除按钮仍重置查询。macOS 与 iPad 最终正式包另行实测搜索提交和清除；完整 IME 与真机时序仍未穷尽。

真实工程使用此前完整采集的 ESP32-S31 缓存离线重放：282,807 条记录，266 个文档，7,569 个二维对象；三维板体 64 × 36 mm，叠层厚度 1.5861538 mm，239 个器件、238 个绑定、34 类模型。索引后板体为 368,214 个顶点和 1,012,656 个索引，GPU 缓冲 17,306,328 字节，Release 几何编译约 241 ms。截图检查板框、开槽、铜、焊盘及丝印；原工程、解密响应和模型素材均未提交。

本机 1440 × 900 离线真实板、器件下载注入失败的浏览器采样：WebKit 首次切换约 1,019 ms，60 次方向键旋转的动画帧回调间隔中位数 12 ms、P95 13 ms；Chromium SwiftShader 对应中位数 115.5 ms、P95 130.9 ms。该指标包含输入与帧回调调度，不代表 GPU 呈现时延或真机 FPS。

| 正式包界面 | 本轮实际操作 |
| --- | --- |
| macOS Apple Silicon | 启动最新 `.app`，打开本地示例，三维板厚、铜线与焊盘可见，拖动旋转和放大正确 |
| iPhone 18 Pro Max / iOS 27 模拟器 | 最新 Release 安装启动成功；三维首帧、旋转、缩放、适应窗口及横屏变化通过。修复前 16-bit depth 导致铜/焊盘被板体遮挡；收紧 near/far 后单测与原生截图确认修复 |
| iPad Pro 11-inch (M5) / iPadOS 27 模拟器 | 最新 Release 安装启动成功；横屏示例三维首帧、旋转、放大及底面视角通过 |
| Android API 34 模拟器 | Release 编译成功。本轮多次安装返回 `Can't find service: package`，即使 `service check package` 短暂显示 found，随后安装仍失败；未取得新版安装、启动或三维界面成功证据 |
| Windows / Linux / 真机 | 本轮尚未完成原生运行和性能验收；推送后的构建状态另行记录 |

在线限制：本轮 `pro.lceda.cn` 元数据请求为 418 或超时；官方 v5 components 响应结构、真实 OBJ 下载和全部底面/复合旋转模型摆放尚未现场验收。OBJ 内嵌材质、装配矩阵和失败重试已用自制夹具覆盖。界面明确报告已加载/缺失器件数量，失败时板体仍可操作。

独立审阅提出的原生窗口尺寸陈旧、GLES3 索引识别、源几何/缺字体警告丢失、部分未闭合板框和 240px 横屏属性卡问题已修复并回归。另发现并修复 iPhone 深度精度问题。按最终审阅保留一项低优先限制：GPU 初始化中途失败可能留存部分 shader/program，直到 context 销毁；正常离开资源释放已有测试。极端自交几何虽有输入/输出数量限制，尚未穷尽 CPU 最坏情况。编辑、三维拾取与制造导出不在本阶段。

代码阶段 `6e0b67e` 与 `35d23c5a8c12194c28b9c776563589dde2f98c95` 已推送到 `main`，HTTPS 读回远端 SHA 一致。[CI #39989576532](https://github.com/gouzil/oshwhub_app/actions/runs/39989576532) 已启动；本次记录时远端 `pnpm check` 已通过，quality 后续步骤和各平台构建仍在执行，尚无整轮成功结论。下面的原生构建、界面操作及 174 项 E2E 是本机证据。

本阶段原生产物 SHA-256：

```text
APK  4bd494555cff6110891add80b412220027d72bffb2a5e270141afe822fe2c0ac
DMG  0a597708273c9da7f18d5f8ee9466f76d8ef4e0be71c62bc97fe4ddfa6205727
iOS main.jsbundle  703bf2e7fc0c7f235e699582c4c8e587d7a77308cdbe5288f277de18e3b95977
```

## 离线检查

| 检查 | 结果与边界 |
| --- | --- |
| `pnpm check` | Biome、TypeScript、31 项 Vitest、12 项构建脚本测试、Rust fmt/clippy/test 通过；Rust 核心 51 项、开发服务 1 项、FFI 1 项，2 项测试默认忽略 |
| `prek run --all-files` | 全部 5 个 hook 通过；已安装本地提交 hook |
| `pnpm test:e2e` | 本轮性能优化最终全量 126/126 通过（2.0 分钟），包括此前偶发的首页极速输入用例。使用自制接口响应，实际加载 CanvasKit WASM 和完整中文字体；历史分阶段结果分别保留在下文 |
| WebKit 补测 | 玻璃导航阶段 1440×900 下 6 项通过、1 项 CDP 透明度模拟显式跳过，4.6 秒；覆盖导航与全部加载回归。上一阶段另有示例板中文/实际像素、铜层隐藏与恢复证据；未在 WebKit 运行 Chromium CDP 手势和生命周期用例 |
| 开发服务超限回归 | 新增真实 TCP HTTP 测试通过：普通非法命令、恰好 1 MiB 成功、超过 1 MiB 拒绝且保留错误码和独立 UUID |
| CodeGraph | 已按用户授权初始化，本地索引不提交 |

E2E 尺寸：320×720、390×844、768×1024、1024×768、1440×900、844×390。覆盖搜索/排序/分页、详情正文、BOM、原页重试、滚动位置恢复、画布像素、中文、缩放按钮/键盘/滚轮、单指拖动、双指缩放、图层隐藏、可见层拾取，以及隐藏列表不再自动分页。图层控件默认可见层优先且切换后不跳位；新增生命周期回归覆盖 StrictMode、连续三次 Fast Refresh、卸载、实际像素及强制 GC 后的 Picture finalizer。新增回归还覆盖 Enter 的当前原生文本、搜索按钮同步草稿、正文真实 PNG 的解码/宽高比和空料号 BOM。无页面横向溢出、未处理的页面错误或非预期 console.error。浏览器采用 SwiftShader；这些结果不等于原生 GPU 和真机验收。

紧凑窗口补测先复现失败再修复：3 个小画布用例验证板子保持在视野内且不因固定留白缩成几个像素；E2E 验证横屏不显示遮挡画布的操作提示，并在窗口进一步缩至 240px 高时确认网络属性与关闭按钮完整可见。

### 本轮物料搜索

Rust 本地筛选与共享搜索输入均先验证测试失败，再实现。核心契约测试覆盖八个字段、中文、大小写、首尾空白、无匹配、空表、关键词边界及原始索引；六个视口下最终 24 项 BOM 回归均通过，53.6 秒，其中新增 12 项搜索回归覆盖匹配数量、快速输入、清除、无结果、原始序号、窗口变化、切板重置、进度动画、过期响应与失败重试。筛选不会重新请求官网工程。`pnpm check` 与全部 5 个 prek hook 通过。

macOS、iOS 模拟器、Android Release 均重新构建成功。macOS 正式包以 ESP32-S31 的 V0.3 板 62 种物料实测：输入带空格的小写 `c15195` 命中 1 项，字段为 10nF / C6 / C0402 / CL05B103KB5NNNC / SAMSUNG(三星) / LCSC / C15195，序号保留为 1；一键清除恢复 62/62。iPhone/iPad 已重新安装并启动新包；Device Hub 的界面自动化连续超时，本轮尚未完成手机搜索的现场交互验收。

Android 安装初次出现 Package Manager `Broken pipe`；模拟器 crash buffer 同时存在 system_server / BatteryStats 系统异常及 `DeadSystemException`。重启 emulator-5554 后安装成功，冷启动返回 `Status: ok`；不把此前的系统服务异常当作应用编译失败，也不把冷启动当作搜索交互验收。此次未重测真实 PCB 手势、真机或超大 BOM 性能。

本轮物料搜索产物 SHA-256（路径见下文原生构建产物）：

```text
APK  f17e0cbac4ec8ed0283211d61d575c67fc5c3a87abd7ecdece62e79be195fba4
DMG  d04b1363ef2b2bbd130c07dec9d24450dbacb2ed33dcb8a15c04c853b8037b2c
iOS main.jsbundle  bb648e8f0e793d0041e79e85afb3c8186e14c4733fea1be25a6bc50f63eec47f
```

三 worker 的首次全量为 101/102，通过搜索新增用例，但横屏正文图片按需加载超时；该用例单独连续 5 次通过。第二次并行全量为 99/102，另有旧的平板分页和桌面极速输入用例超时。截图显示图片用例滚到了图片后的附件区域；桌面极速输入失败时输入框缺少最后一个字符。最终 `pnpm test:e2e --workers=1` 为 101/102，5.3 分钟：BOM 24 项全部通过，首页极速输入用例在 phone-390 下仍失败，输入框只保留部分字符；该首页用例另行重复 5 次为 4 通过、1 失败。尚未定位并修复其根因，不能宣称全量回归通过；首页和正文懒加载测试、首页实现均未在本阶段修改。

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

### 系统 Tab Bar、选中层与 Android 折射材质

用户反馈手机和桌面材质均不够接近系统效果。本次 iOS 窄屏改为已有 Expo 模块注册的真正 `UITabBar`，由 UIKit 提供 SF Symbols、选中镜片和触摸反馈；iPad 宽屏侧栏采用 clear 材质。共享侧栏降低白底不透明度、加入透光渐变和细亮边，替换字符图标，选中层按实际导航项边界移动。Android 新接入固定版本 `expo-android-glass-view` 1.0.2，以 RenderNode 背景缓存和 AGSL 提供折射与模糊。候选库比较、兼容范围和性能取舍见 [design.md](design.md#android-玻璃材质选型)。

最终源码的 `pnpm check`、全部 5 项 prek hook 通过；Chromium 六个视口 78/78 E2E 通过（1.2 分钟），包含选中层边界对齐、暂停 JS 动画帧、减少动态效果和降低透明度回退。生产 Web 构建在 Chromium 与 WebKit 的导航补测为 5 项通过、1 项 CDP 模拟显式跳过（6.9 秒）。新增暂停动画帧用例先复现 JS 弹簧不再推进，再验证直接更新 CSS 目标位置；最终 Web 使用 CSS left/top 过渡，原生选中层使用原生驱动 transform。

iPhone 18 Pro Max / iOS 27.0 模拟器实际看到系统胶囊、灰色选中镜片和按压反馈，切页及进入 PCB 后底栏隐藏通过。iPad Pro 11-inch (M5) / iPadOS 27.0 竖屏使用系统底栏、横屏恢复侧栏，保留当前页面。最终 iOS Release 重新构建成功，并安装、启动到两台模拟器。

macOS 最终 `.app` 与 DMG 均重新构建成功。解锁后用户在正式包前台手动确认「PCB 实验室 → 关于」白色胶囊正常跟随，随后实际正式包截图也显示镜片与 PCB 实验室选中项对齐。后台自动化曾见页面/文字已切换而镜片停留在上一项；临时诊断取得正确的项布局和目标 inline 样式。CSS transform 与 left/top 都曾在后台截图中停留，不能把现象归因于 transform 本身，也不能用后台截图或浏览器通过代替前台验证。没有保留调试文本、周期性强制布局或刷新定时器。此前锁屏时 DMG 打包卡在 Finder 的尝试已被本次成功产物替代；后台恢复的长期稳定性仍需补测。

Android Release 在 Expo 58 / RN 0.88 下构建成功（含新库 Kotlin/Compose），安装到 API 34 的 emulator-5554 并冷启动。真实信息流中可见图片颜色透射、边缘折射、线条图标与独立选中层；点击 PCB 实验室后，页面文字与原生 selected 状态一致。临时把 `transition_animation_scale` 从 1.0 改为 0，导航退为实色，非图标区域 16×16 像素全部匹配 `#f6f9f7`（每通道容差 1）；点击关于后正文和 selected 状态正确，随后恢复设置为 1.0。此测试检查实际像素和交互，不以原生 View 类名判断材质。 从 PCB 实验室打开本地示例后，原生树中玻璃导航已移除；返回后玻璃面板和 PCB 实验室选中态恢复，本段回归的崩溃日志为空。此次只补验导航卸载/恢复，不重复声称完整重测真实 PCB 的所有操作。

静置 PCB 实验室约 48.6 秒，`dumpsys gfxinfo` 总帧数保持 106，支持该静态页面没有持续重绘。模拟器使用 SwiftShader 软件 GPU，动态样本与实色对照均出现秒级帧耗时；它们不能证明真机帧率、功耗或组件之间的性能排名。首次着色器编译、内容更新和虚拟机调度也混在该短样本内；需用真实 Android GPU 对固定输入单独测量。

最终产物 SHA-256：

```text
APK  467404fa150e02405f036e6082851ef69c629dd33ce1ee4ba17397fec961fdd2
DMG  8b378465dedd326563376e44a6e66c6a2ff7621b8853f300096f279291e1cc2c
iOS main.jsbundle  c38f215aa14ff7a98ed269effb84fea3760e3d8c5cb13b372999fd959578e24f
```

未补测旧 Android/iOS 系统、真机 GPU/功耗、VoiceOver/TalkBack、Windows/Linux 原生外观；桌面仍是 CSS 近似，Android 为第三方 GPU 实现，均不宣称与 UIKit 材质完全相同。

### BOM 字段与可读性

对照 [官网实战派 ESP32-S31 工程](https://oshwhub.com/li-chuang-kai-fa-ban/project_wxwsirbz)，展开的 BOM 有 62 行物料、63 个表格行（含表头），没有物料图片元素；其公开 BOM 字段也没有图片地址。本次按官网字段拆开名称 / 参数、位号、封装、数量、制造商型号、制造商、供应商和供应商料号。详情可用宽度达到 720px 时使用中文表头宽表，手机使用标签和值逐行对应的卡片；空字段显示“未提供”，长值完整换行，数量 0 保留。

新增 Rust 契约测试先复现制造商/供应商字段丢失，再验证独立映射、缺失字段和数量 0。六个视口各增加 2 项 BOM 回归，验证字段和值的无障碍对应、空值、长位号/型号、表内横向滚动、键盘操作、宽桌面全部列可见、无全页横向溢出，以及改变窗口尺寸后保留所选板子。最终 `pnpm check`、5 个 prek hook 和全量 90 项 Chromium E2E 通过；新增 BOM 截图使用自制数据。

Android（含 arm64-v8a/x86_64 Rust 库）、iOS 模拟器和 macOS Release 均重新构建成功。macOS 正式包打开上述真实工程，核对 62 种物料 / 231 个元件、中文表头、长位号换行和独立厂家/供应商字段；首项为 10nF、C6、C0402、数量 1、CL05B103KB5NNNC、SAMSUNG(三星)、LCSC、C15195。iPhone 18 Pro Max / iOS 27.0 安装并启动最终包，真实物料卡片的标签和值、缺失字段及第 62 项末尾可见。iPad Pro 11-inch (M5) / iPadOS 27.0 安装同一包，实际宽表中的长封装换行正常，横向滑动后供应商和料号完整可见。Android 最终 APK 安装到重新启动的 emulator-5554，冷启动返回 `Status: ok`，本轮没有现场逐项核对 Android BOM。

本轮未重测全部原生 PCB 手势、VoiceOver/TalkBack、极大字体或上千种物料的性能；BOM 仍一次挂载，未新增虚拟化。Windows/Linux 的新构建结果以本次提交的 CI 为准。

本次 BOM 修改最终产物 SHA-256：

```text
APK  ea8321a790980fb6ae0b91d83f6c8f5a34807543ec19a3703238678723a2510e
DMG  f06e3fd8297dfb3ffbf873e06a82a736cd530154f0cdad847574a4af659c8ba3
iOS main.jsbundle  4359e0d7447246eadb8f43f7c0536ceb0b054d51d6820479ead0b0144874b2d7
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

## PCB 工作区改版验证

对照 Shapr3D 官方 macOS、iPadOS 工作区及 Items Manager 实图调整共享查看器，设计来源和范围见 [design.md](design.md#pcb-工作区参考-shapr3d)。画布占满标题栏下的安全区域，工具沿边悬浮；图层、信息、拾取属性一次展开一个面板，手机窄屏贴底，平板和桌面靠右。开关面板不改变画布尺寸或重新适应视口。

最终 `pnpm check` 通过（29 项 Vitest、12 项脚本测试、Rust 核心 51 项、开发服务和 FFI 各 1 项，2 项忽略），全部 5 个 prek hook 通过。同机并行构建三个 Release 时，有一次字体录制测试超过原有 5 秒限制；构建结束后未改测试或超时设置，完整 `pnpm check` 复核通过。

新增面板回归在六个视口下全部通过，检查应用导航隐藏/返回恢复、画布占用、面板展开状态与互斥、关闭后保留视口和图层选择、信息和拾取属性。最终查看器相关 18 项 Chromium E2E 全部通过（47.3 秒），涵盖真实 CanvasKit/中文字体、铜层像素、图层顺序、拖动/双指/滚轮/键盘缩放。极短横屏先复现信息按钮只有 95.45% 在视口内，再改为横向工具组；844×240 下信息按钮、网络属性和关闭按钮完整可见，红绿回归通过。真实板的同尺寸截图也确认工具与面板没有裁切。

本次改版的全量回归为 107/108（1.6 分钟）。唯一失败是 `wide-1440` 的首页 `Enter submits current native text and rapid typing supports the search button`，输入 `ESP32-S31-USB` 后文本缺少字符；物料搜索阶段已记录同一问题，本阶段未修改首页搜索实现。最终短横屏修复后只重跑了上面的 18 项相关回归，不能将其当作全量通过。

macOS `.app` / DMG、iOS Simulator Release、Android Release 均在最终源码上重新构建成功。macOS 退出旧进程后重开最终正式包，实际打开 ESP32-S31 的 V0.3 板（`90602f2f840d4245a97479e90a31d17b` / `d4c95e111c8e5672`）：7,569 图元、62 图层、默认可见 9 层，右侧图层面板及信息面板互相切换正常，4 项格式显示提示可读。浏览器另查同一真实板的桌面、手机窄屏和 844×240 极短横屏；手机底部面板关闭后恢复完整视野。

iPhone 18 Pro Max / iOS 27.0 安装并启动最终 Release，打开本地示例后确认深色状态栏安全区域、底部导航隐藏、图层列表与颜色/名称/状态分列；关闭 Bottom Copper 后蓝色曲线消失，关闭面板保留 5/6 层状态，放大到 125% 正常，返回后系统底栏和 PCB 实验室选中状态恢复。iPad Pro 11-inch (M5) / iPadOS 27.0 安装同一包，确认画布全宽和右侧图层面板。Android 最终 APK 安装至 emulator-5554 成功，冷启动 `Status: ok`；本阶段未完成 Android 查看器现场交互验收。

最终产物 SHA-256（产物路径见“原生构建与运行”）：

```text
APK  e047ee2316fa2b06ff0d438e2a98e34482303b0f048ea4ab6bd172c54daedf53
DMG  69111624f738cfe29d1ca6a97002808dd26c4ed35eed343e1a0b9a01d7f748ec
iOS main.jsbundle  4b5d40b84255d750d3456f870d1f89dd17386b452cfadd6b50c1e82392334ee3
```

本次没有改动 Rust 解析/拾取或 Picture 绘制算法，未重复真机 GPU/功耗/内存基准、VoiceOver/TalkBack、极大字体和 Windows/Linux 原生视觉检查；模拟器与浏览器布局结果不代表这些项目已验收。PCB 仍为二维只读查看器。

## PCB 交互性能优化

用户反馈所有板子的缩放、平移和选择都卡顿。本轮定位到两个共享路径：每次相机变化重新重放可见层的完整矢量 Picture，且 scale/x/y 三次 shared-value 写入触发重复提交；菜单和属性状态变化也会重建 Canvas。现在操作期间绘制有界全板/视区图片，一次更新完整相机，停止 180ms 后从原始矢量补充细节，面板变化复用 Canvas。完整数据、原始图层和 Rust 几何拾取保留；缓存预算、生命周期与暂时模糊边界见 [renderer.md](renderer.md#interaction-raster-cache)。

### 相同真实板的 WebKit 基准

本机 Playwright 无头 WebKit，1440×900、DPR 2，同一 ESP32-S31 V0.3 PCB（7,569 图元、62 层、默认可见 9 层），等待首帧后在画布中心依次发 20 次 wheel(0,-25) 和 20 次 wheel(0,25)，在发送操作期间采样 `requestAnimationFrame` 间隔及 CanvasKit `drawPicture` / `flush` 同步调用。前后同环境、同输入，计时不含网络下载和首次缓存构建，也不含输入结束后的空闲细化。

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 40 次输入完成耗时 | 50,738 ms | 472 ms |
| rAF 间隔 P50 / P95 | 428 / 1,759 ms | 12 / 13 ms |
| flush P95 / 最大值 | 331 / 670 ms | 2 / 3 ms |
| flush 调用次数 | 118 | 40 |
| drawPicture 调用次数 | 1,882 | 40 |

这些是一次受控浏览器样本：rAF 不是屏幕实际呈现统计，flush 同步耗时不等于 GPU 完成时间，输入完成耗时还包含自动化调度。不能由此声称 iPhone、Android 或 macOS 正式包达到同样 FPS。真实场景响应只用于本地测量，没有提交到仓库。

### 回归与原生包

缓存重放回归先在旧路径失败，再在新路径通过；不使用受 CI 机器负载影响的绝对 FPS 阈值。新增测试验证操作帧不重复重放每层矢量，开关面板不重绘板子，931% 缩放停止后创建细节，平移越过旧细节边界后的中心像素与实际图层一致，隐藏图层使旧缓存失效。一次性注入分配失败先复现错误无法恢复，再修复并验证下次图层重建恢复画布。图片回收检查扩展现有 StrictMode / 三次 Fast Refresh / 卸载测试，以实际像素和强制测试 GC 后的 Image finalizer 检查所有权；六视口最终相关 E2E 24/24 通过（52.7 秒）。

最终 `pnpm check` 通过（31 项 Vitest、12 项脚本测试、51 项 Rust 核心及开发服务/FFI 各 1 项，2 项忽略）。最终全量 Chromium E2E 126/126 通过（2.0 分钟），包括已有首页、正文懒加载、BOM、导航和查看器用例；之前记录的首页极速输入用例此次通过，不据此宣称其历史偶发性已定位或修复。

最终源码重新构建 macOS `.app` / DMG、iOS Simulator Release、Android Release，均成功。Android 构建初次缺少 ANDROID_HOME，使用本机已有 SDK 后通过；APK 安装到 emulator-5554 成功，冷启动 `Status: ok`。

| 平台 | 本轮现场检查 |
| --- | --- |
| macOS Apple Silicon Release | 退出旧进程后打开新包，真实 ESP32-S31 板连续滚轮放大到 1678%、拖动、缩小至 100%、点选属性成功。全板缓存 2047×1410 / 56ms，停下后的视区细节 2668×1571 / 23ms，边缘与孔洞可见 |
| iPad Pro 11-inch (M5) / iPadOS 27 模拟器 Release | 同一真实板，连续放大到 931%、拖动、Top Layer 隐藏/恢复后的像素正确，复位并点选 STRING 成功。全板缓存 2047×1410 / 21ms，视区细节 1784×2349 / 15ms |
| iPhone 18 Pro Max / iOS 27 模拟器 Release | 本地示例连续放大到 931%、拖动、信息面板、复位和 PAD / 3V3 属性成功；关闭 Bottom Copper 后蓝色曲线消失，返回后系统导航恢复。全板缓存 2048×1430 / 3ms，视区细节 1518×2761 / 5ms |
| Android API 34 / SwiftShader 模拟器 Release | 安装、冷启动成功，本轮未取得原生手势/帧时间验收结果 |

上述缓存构建时间均为单次样本。尝试 Instruments Animation Hitches 附加 iOS 模拟器时返回 `Hitches is not supported on this platform`，没有得到可用原生 FPS 统计。本轮未测真机持续手势、功耗和长时间峰值内存，也未复验 Windows/Linux 原生界面；模拟器功能与 WebKit 采样不能替代这些测量。

本轮最终产物 SHA-256：

```text
APK  f9212ac0ae1e30e2a0d2da89563be3d0e332925dc5fb354f615f22aeb749eb4b
DMG  5b166946fc6bb886d7f9ee85534804a61ad2923ea3c608a9b28e7c434acb9522
iOS main.jsbundle  d5132bfc72637d766e159f74caa7173e4c15a03e994a99cf1f40589198717e20
```

## 图层切换与选中属性改进

切层闪屏的直接原因是可见 Picture 数组变化后，界面先卸载 Canvas、显示加载指示，再用全板缓存和延迟细节重新挂载。现在同一场景保持 Canvas，离屏重建全板与已有视区细节后一起发布，相机不变；可选细节分配失败时发布新全板、记录警告，基础缓存分配失败仍显示可恢复错误。没有按图层新增高分辨率图片缓存。

选中卡片优先显示位号或焊盘编号，副标题显示对象类型和图层，正文以中文标签展示网络、参数及封装名称。原始 ID、ticket 和引用默认折叠，重新点选时收起。卡片高度随内容变化，窄屏位于底部，长属性换行且关闭始终可达。Rust 从已有实例与封装元数据补充拾取属性，并保持 board ATTR 覆盖表独立，避免误隐藏继承标签。

回归测试先在旧实现复现 Canvas 被卸载和属性不可读，再验证新路径。独立审查发现的继承标签被隐藏、可选细节分配失败导致整板不可见两条路径，也分别用 Rust 可见文字断言和 CanvasKit 分配失败注入完成红绿验证。新增 E2E 检查 Canvas 原节点存活、切层后的实际像素、细节缓存保留或降级、C49 可读属性、技术详情收起和长文本布局；六种尺寸上的全部 PCB 相关用例通过。

最终 `pnpm check` 通过：35 项 Vitest、12 项脚本测试、52 项 Rust 核心测试，开发服务和 FFI 各 1 项，2 项忽略。`prek run --all-files` 全部通过。完整 Chromium E2E 为 143/144 通过（2.0 分钟），失败仍是此前记录的 desktop-1024 首页极速输入搜索；单独复验 1/1 通过（4.5 秒）。真实板复查时又补充折叠 `Channel ID` / `Unique ID`，最终源码的 PCB 与选中卡片专项六视口 36/36 通过（50.5 秒）。本阶段没有修改或声称修复该搜索偶发问题。

实际界面检查使用重启后的 Rust 开发核心与浏览器界面，重新从公开搜索进入 ESP32-S31 V0.3（7,569 图元、62 层）。195% 缩放下隐藏/恢复 Top Layer 后，铜面变化正确且相机不重置；点选 R41 的焊盘显示“R41 · 焊盘 1”、网络 `$4N53`、封装 `R0402`。展开技术详情能查看原始属性，收起后内部关联 ID 不再出现在主卡片。截图保存在本次 Codex 可视化目录，未提交真实工程响应。

修复后的源码已重新构建 macOS `.app`/DMG、iOS Simulator Release 和 Android Release，均成功。iPhone/iPad 模拟器安装成功；Android emulator-5554 安装成功，冷启动 `Status: ok`。首次原生补验曾被 Mac 锁屏阻挡；2026-10-23 解锁后，使用相同正式产物完成以下检查，没有沿用上一阶段的设备界面结论：

| 运行环境 | 本阶段原生界面补验 |
| --- | --- |
| macOS Apple Silicon Release | 从公开搜索重新打开真实 ESP32-S31 V0.3（7,569 图元、62 层）。195% 下隐藏/恢复 Top Layer，铜面变化正确，视图位置和比例保持不变；复位后点选显示“U1 · 焊盘 2”和中文“封装 / ANT-SMD_L3.2-W1.6”。技术详情可展开、关闭，重新点选后收起 |
| iPhone 18 Pro Max / iOS 27 模拟器 Release | 本地示例放大到 596% 并拖动，隐藏/恢复 Bottom Copper 后蓝色圆弧正确消失/恢复，相机保持不变。底部紧凑卡显示“焊盘 1”“网络 / 3V3”，关闭按钮可达；展开技术详情后可关闭，重新点选默认收起 |
| iPad Pro 11-inch (M5) / iPadOS 27 模拟器 Release | 同一真实 ESP32-S31 V0.3，244% 下隐藏/恢复 Top Layer，视图比例和位置不变。点选 U1 焊盘后的侧卡字段、技术详情展开/关闭及重选收起正确；旋转到横屏后仍显示当前选中项，关闭入口和缩放工具可见 |
| Android API 34 / SwiftShader 模拟器 Release | 沿用本阶段安装与冷启动结果，本轮未补验原生手势、切层动画和属性卡外观 |

macOS 前两次真实工程请求分别在 34,004ms 和 33,154ms 后返回可重试的 `TIMEOUT`，再次重试成功；成功日志记录 23 段历史、9,906,972 字节、282,807 条记录，加载总计 37,190ms，其中解析 438ms、场景构建 71ms。现有失败日志没有定位到具体上游请求，不能据此归因；该总耗时也不是画布交互帧时间。未修改超时策略。

三端属性卡及切层前后截图保存在本次 Codex 可视化目录，未提交真实工程响应。本轮仅补原生验收与文档，未改源码或重复全套本机测试；产物校验值与前次构建一致。仍未取得真机持续手势 FPS、功耗和峰值内存数据，也未补验 Windows/Linux 原生界面。

本阶段最终产物 SHA-256：

```text
APK  a8a6dc364749d6a888d96e81a7ff8442da3c3e72f6892fff4705d74a5a80bc53
DMG  ad8e781cdae170c553272e7ff2bf59c2e263631dd2fa61d0d2f7241efd0a2a91
iOS main.jsbundle  dab8e611c510a7ce7473da20fa0497106a977aa0b7f18f53619be2668cf3cd10
```

## 交付与后续

阶段提交已推送至 `main`：`b6afb5b` 设计、`9ecbdb7` 独立渲染方案、`c374034` Rust 核心、`c81d117` 共享客户端、`3dcf55c` CI/质量、`b5b7e0a` 实际 UI 回归与 Android SDK 安装修复、`fa4f534` PCB 内存优化、`51dd1af` Android 共享源码增量打包、`7abf60d` Picture 生命周期与图层操作、`2d20ad0` 验收记录、`6f7bbf8` 紧凑画布与属性面板适配、`d77063b` 原生搜索和正文兼容、`1ac8b68` 骨架、正文按需图片与长海报修复。

首轮 [CI #39830220901](https://github.com/gouzil/oshwhub_app/actions/runs/39830220901) 的 quality、iOS 和三个桌面平台均成功；Android 在过时 setup-android v3 的 tools 包安装阶段失败。已升级 v4 并显式只安装 platform-tools。

第二轮 [CI #39833066803](https://github.com/gouzil/oshwhub_app/actions/runs/39833066803) 的 quality、Windows、Linux 成功。macOS/iOS 因 hosted runner 容量不足未启动并取消；Android 在原生依赖编译中收到 runner shutdown 信号而中断，日志没有源码编译错误证据。本机对应 Android/iOS/macOS Release 构建通过。

代码 `7abf60d38568a4589228a11f8a02c45750fb0bff` 的 [CI #39839629388](https://github.com/gouzil/oshwhub_app/actions/runs/39839629388)：quality、iOS 和 Windows/macOS/Linux 三个桌面构建全部成功，平台产物已上传。quality 中统一检查、prek、Web 构建及 30 项 E2E 通过（CI E2E 3.1 分钟）。Android 的 SDK、Rust 双架构和原生工程生成已成功，Gradle Release 未取得成功结论；新代码推送后已取消该过时运行，最终状态为 cancelled，不能当作源码编译失败。

代码 `6f7bbf8268589a93195e7aa13e0a823460bb6402` 的 [CI #39846343412](https://github.com/gouzil/oshwhub_app/actions/runs/39846343412) 已结束：quality、Windows、Linux 成功，Android 失败，macOS/iOS 取消；整轮为 failure。quality 日志确认 27 项 TS 测试、7 项构建脚本测试、51 项 Rust 测试及 30 项 E2E 通过（CI E2E 2.9 分钟）。

代码 `d77063b1eddcd15d36a630dd9a461acb098ca2b6` 的 [CI #39881811212](https://github.com/gouzil/oshwhub_app/actions/runs/39881811212) 已结束：quality、iOS、Windows、Linux、macOS 全部成功；Android 在 SDK manager 下载 NDK 27.1.12297006 时出现 `Error reading Zip content from a SeekableByteChannel`，尚未进入 Rust/Gradle 构建，整轮为 failure。该结果不能当作 Android 源码编译失败。

代码 `1ac8b68286b12a9142fe65b9d59d788ff804c635` 已推送并触发 [CI #39890604300](https://github.com/gouzil/oshwhub_app/actions/runs/39890604300)。2026-10-23 本轮记录时仍在执行；上面的 60 项 E2E、WebKit 6 项、统一检查、prek 及 Android/iOS/macOS Release 均为本机结果。文档提交跳过 CI，避免取消正在执行的代码构建。

玻璃导航代码 `c6d3184c74abf78f811440436f81245154390dc8` 已推送至 `main`，HTTPS 读回远端 SHA 一致；[CI #39894586310](https://github.com/gouzil/oshwhub_app/actions/runs/39894586310) 已触发，记录时为 pending，尚无成功结论。此前加载优化 CI 的 quality、iOS 和三个桌面构建均已成功，Android 仍停留在 Gradle Release 构建中；本轮 72 项 E2E、WebKit、prek 与三个本机 Release 的证据见“本轮玻璃导航”。

图层切换与选中属性代码 `6f0f9675b5eab516de7c54d8ed50c58e09d624f9` 的 [CI #39957917251](https://github.com/gouzil/oshwhub_app/actions/runs/39957917251) 已结束，整体为 `success`：quality、Android、iOS 和 Windows/Linux/macOS 三个桌面构建全部成功。2026-10-23 原生补验时再次通过 GitHub API 核对了运行 SHA 和各任务结论。

系统已解锁，本轮 macOS 正式包界面与外链验证已继续完成；此前锁屏阻挡已解除。文档更新与源码构建证据按各自版本区分。

本轮不宣称覆盖所有专业版文档类型或全部设备。重点后续项为真机性能采样、复杂板的首帧/帧时间/内存基准，以及文档中明确提示的格式兼容扩展；账号功能按用户确认另行迭代。
