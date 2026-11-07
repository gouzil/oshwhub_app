# NSH 性能对照数据格式

`nsh-performance.mjs` 只比较已经采集并保存的原始数据。它不会启动应用、采样设备、合成样本或把 headless/软件 GPU 结果当作硬件结果。每个输入文件都是一个 JSON capture；第一个参数必须是 `role: "baseline"`，第二个必须是 `role: "candidate"`。

提交文件使用 `exactCommit` 保存精确提交标识；为兼容采集器也接受同义的 `exactcommit`，两者同时出现时必须相同。

运行：

```sh
node scripts/nsh-performance.mjs baseline.json candidate.json [report.md]
```

退出码是 `0=pass`、`1=fail`、`2=inconclusive`、`3=invalid`。无效输入不会产生可通过的部分报告。

## 顶层结构

```json
{
  "schemaVersion": 1,
  "role": "baseline",
  "exactCommit": "40-character-or-other-opaque-exact-commit-id",
  "metadata": {
    "device": "MacBookPro18,3",
    "os": "macOS 15.7",
    "gpu": { "name": "Apple M3 Max", "kind": "hardware" },
    "backend": "metal",
    "refreshHz": 60,
    "dpr": 2,
    "viewport": { "width": 1440, "height": 900 },
    "inputSha256": "64-lower-or-upper-hex-characters",
    "cache": "cold",
    "release": true,
    "metricDefinitions": { "...every metric name...": "definition and sampling unit" },
    "sampling": {
      "method": "independent-run-bootstrap",
      "startupRuns": 10,
      "interactionGroups": 5,
      "interactionDurationSec": 60,
      "lifecycleCycles": 20,
      "bootstrapReplicates": 2000,
      "confidence": 0.95
    }
  },
  "scenarios": {
    "pages": { "sample": { "label": "...", "kind": "real-board" }, "startup": {}, "interaction": {}, "lifecycle": {} },
    "pcb2d": { "sample": { "label": "...", "kind": "synthetic-stress" }, "startup": {}, "interaction": {}, "lifecycle": {} },
    "schematic": { "sample": { "label": "...", "kind": "example" }, "startup": {}, "interaction": {}, "lifecycle": {} },
    "pcb3d": { "sample": { "label": "...", "kind": "real-board" }, "startup": {}, "interaction": {}, "lifecycle": {} }
  }
}
```

`sample.kind` 必须是 `real-board`、`synthetic-stress` 或 `example`，每个场景必须有可读的 `sample.label`。合成压力数据只能作为标注为合成的压力场景，不能代表真实板性能。baseline 与 candidate 的样本标签和种类、输入 SHA、设备、系统、GPU、后端、刷新率、DPR、视口、缓存状态、Release 标志、指标定义和采样方法必须逐字段相同；只有 `role` 和 `exactCommit` 应不同。`gpu.kind` 必须为 `hardware`；SwiftShader、llvmpipe、软件 rasterizer 等名称会被拒绝，即使调用方把 kind 写成 hardware。

`metricDefinitions` 必须解释以下所有名称：`startupMedianMs`、`startupP95Ms`、`frameP50Ms`、`frameP95Ms`、`frameP99Ms`、`dropRatio`、`dropP50Ratio`、`dropP95Ratio`、`dropP99Ratio`、`cpuPct`、`rssBytes`、`gpuBytes`、`layerP95Ms`、`pickP95Ms`、`bomP95Ms`、`rssGrowthBytes`、`gpuGrowthBytes`、`rssTrendBytesPerCycle`、`gpuTrendBytesPerCycle`、`gestureCompileCount`、`pathParseCount`、`meshUploadCount`。所有数值都必须是有限 JSON numbers；NaN、Infinity、null、负的时延/内存/计数会使文件无效。

## 场景采样

每个场景的 `startup.runs` 至少 10 个独立冷启动对象，每个对象为 `{ "durationMs": number }`。`interaction.groups` 至少 5 个独立组，每组至少 60 秒：

```json
{
  "durationSec": 60,
  "frameMs": [16.4, 16.7, 17.1],
  "droppedFrames": 0,
  "cpuPct": 24.1,
  "rssBytes": 123456789,
  "gpuBytes": 456789012,
  "actions": { "layerMs": [8.2], "pickMs": [3.1], "bomMs": [12.0] },
  "gesture": { "compile": 0, "pathParse": 0, "meshUpload": 0 }
}
```

上面的 `frameMs` 仅展示数组格式，实际文件必须保留整组观测。`frameMs` 是连续呈现到呈现的正帧间隔，总和须与 `durationSec * 1000` 相差不超过一个刷新间隔；只有开头和结尾采样边界允许这点误差，截断或重复的帧记录会被拒绝。每组使用持续的交互回放，不把等待网络、预热或人为暂停计入这 60 秒；需求驱动绘制中两个呈现之间的停顿仍须保留为完整间隔，不能删掉或填入虚构帧。`sampling.confidence` 固定为 `0.95`。`dropRatio` 由 `droppedFrames / (frameMs.length + droppedFrames)` 计算。`layerMs`、`pickMs`、`bomMs` 是各操作的原始时延数组，比较器在组内取 p95、再以独立组为 bootstrap 单位。每组的工程编译、路径解析、网格上传计数必须都为 0，任何非零值都会使 capture 无效。

`lifecycle.cycles` 至少 20 个打开/关闭循环：

```json
{
  "rssBeforeBytes": 123456789,
  "rssAfterBytes": 123456789,
  "gpuBeforeBytes": 456789012,
  "gpuAfterBytes": 456789012
}
```

`rssGrowthBytes` 和 `gpuGrowthBytes` 是每循环 after-before 的 p95。比较器还报告 after 序列的线性 `*TrendBytesPerCycle`；任一版本存在严格正的持续增长都直接失败，不能用“双方都增长”抵消泄漏。生命周期样本必须正好保留每次循环的原始值，不能只提交一个汇总数字。

## 统计和判定

启动指标由独立启动 run 计算 median 和 p95。交互指标由独立 60 秒组计算 frame p50/p95/p99、丢帧比例的 p50/p95/p99、CPU/RSS/GPU 峰值的 p95 以及切层/拾取/BOM p95。比较器以这些独立 run/group/cycle 为重采样单位，使用固定哈希种子的确定性 percentile bootstrap（默认 2000 次，95% CI），绝不把单帧或同一组内的操作当作独立 run。

所有指标都是越低越好。candidate 的 bootstrap delta 为 `candidate - baseline`：对于非完全相同的数据，CI 上界 `< 0` 才能通过；CI 下界 `> 0` 失败；边界触及 0 或跨过 0 都是 `inconclusive`，不能用 5% 或 10% 阈值“放行”。当且仅当两边的独立 run 数组逐项完全相同，比较器使用 `[0, 0]` 的退化 CI 报告 `exact equal independent-run estimates`；仅仅四舍五入后相同、均值相同或样本有噪声都不算 equal。

报告同时列出 baseline/candidate 估计、delta、95% CI、判定和原因，并列出每个场景的 sample label/kind 与完整设备 metadata。缺少任一场景、样本数量不足、元数据不一致、非 Release、软件 GPU、非有限值、非零手势工作或不符合 20 次生命周期要求都会返回 invalid（退出码 3），不会被降级成 inconclusive 或 pass。

该工具的输出不是性能证明：真实设备/原生 GPU/Release 采集、命令、原始样本和未测平台仍必须分别保存并审阅。合成和示例场景的标签会保留在报告中。

## macOS 进程诊断

```sh
node scripts/nsh-process-sample.mjs 12345 65 output/macos-process.json
```

参数依次为已启动且预热完成的应用根 PID、采样秒数和不存在的输出文件。采样器约每秒读取一次
macOS `ps`，保存目标及全部子孙进程的累计 CPU 秒数、RSS、启动时间和采样耗时。
`cpuPct = CPU 秒数增量 / 墙钟秒数 * 100`，一个持续占满的核心为 100%；CPU 原始计数精度为
0.01 秒。`peakRssBytes` 是每次采样的整棵进程树 RSS 之和的最大值，包含重复计算的共享页，
不等于物理 footprint 或 GPU 分配量。启动时间用于识别 PID 重用。

缺少根进程、观测到进程树变化、计数倒退或采样失败时，文件保留已取得的样本并记录 `error`，
退出码为 1；已有文件不会被覆盖。轮询无法发现两次采样之间就已结束的短命子进程，因此该工具
只用于预热后进程稳定的交互诊断，不用于冷启动或有进程变化的生命周期验收。

输出标为 `kind: "process-diagnostic"`，不提供帧、丢帧、GPU 内存或操作时延，不能直接输入上面的
验收比较器。必须另外保存输入动作及时间边界；包含等待或工具调用间隙的诊断，不能算作持续交互组。
