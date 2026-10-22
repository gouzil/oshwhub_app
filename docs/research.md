# 研究依据与证据边界

日期：2026-10-23（Asia/Shanghai）。所有结论仅覆盖检查到的版本和公开工程。

## 前期讨论

已通过 `read_thread` 读取本机 chat `01a16280-efab-7db3-bd58-8a90a7714561`。其中记录的接口和格式作为研究线索，实际实现重新请求与校验。

前期关键发现：OSHWHub 工程详情与 `proImages` 提供内容、板子和缩略图；完整设计记录来自专业版历史接口。独立二维查看器 [easyeda/easyeda-viewer](https://github.com/easyeda/easyeda-viewer) 与专业版安装包中的 `smt-gl-engine` 是不同实现。

## 本机参考

- APK：`app_zh_release.apk`，通过 Android SDK `aapt dump badging` 确认为 Bambu Handy 4.3.0（versionCode 40300，package `com.bambulab.creator`），不是 OSHWHub 客户端。
- 模拟器：`emulator-5554`，arm64。已观察首页实际界面：顶部搜索和分类、双列卡片、底部导航。仅用作设计参考。
- 嘉立创专业版：本机安装包包含 `smt-gl-engine 4.1.20.b63c94d5`。不将它的依赖许可证解释为整个引擎的复用许可；本项目不包含该闭源引擎。

## 公开 API 和设计记录

待实际请求和测试完成后，在此记录精确端点、字段、版本与限制。实时响应与以前保存的样本不能混作同一次请求。

## 第三方代码

官方二维查看器固定到提交 `1c841a7fdcdc382f7a516a4f5a261ad34cb8162d`（仓库标记版本 0.5.0），采用 Apache-2.0。实际 vendoring 时保留 LICENSE、来源和修改说明。工程样本有独立内容权利，测试优先使用自制小型夹具，真实工程只在授权边界内读取，不擅自更改其许可证。
