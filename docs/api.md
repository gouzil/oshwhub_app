# 公开 API 契约

本轮只查询公开数据，不发送登录态、账号写入、浏览历史或计数上报。Rust 模型是桥协议的 camelCase JSON；官网 snake_case 字段在核心归一化。正文不会放进 WebView 执行。

## 请求与分页

- 浏览：`GET https://oshwhub.com/api/project`，参数 `page/pageSize/type/origin=all/projectTag=/keywords=/public=true`。
- 搜索：`POST https://oshwhub.com/api/search/data`，JSON 字段 `type=project, keywords, category="", sort, page, pageSize`。它是查询接口。GET 列表关键词筛选的覆盖范围不同，不能替代正式搜索。
- 页码从 1 开始，边界为 1–10000；每页 1–50；关键词去除首尾空白后最多 100 字符，拒绝控制字符。排序 `recommend/new/like` 分别为推荐、最新、点赞；搜索推荐映射空 sort。
- `page/pageSize/total/totalPage/lists` 来自官网响应。归一化按 UUID 保留第一次出现的记录；`hasMore` 按页元数据计算，不依赖去重后的行数。
- 详情：`GET /api/project/{id}`。专业版再读取 `/api/project/{id}/proImages`；标准版此端点返回 104006，因此只展示详情，不错误调用专业版预览接口。

## HTTP 边界

只允许 HTTPS、443 端口、无用户名密码以及明确允许的 exact hostname。当前域名为 `oshwhub.com`、`pro.lceda.cn`、`modules.lceda.cn`、`image-pro.lceda.cn`、`image.lceda.cn`、`static.oshwhub.com`。不跟随重定向，避免允许域名跳转到任意目标。

连接超时 5 秒，总超时 30 秒；JSON 最多 8 MiB；二进制下载流式累计并执行调用者大小上限。JSON 必须返回 JSON MIME；HTML 拦截页、418、429 和业务错误都作为明确错误返回，不变成空页。PCB 缩略图实际可返回 `application/octet-stream`，二进制下载允许此 MIME。日志只保留方法、域名、状态和耗时，不记录完整 URL、查询、正文或密钥。

## 数据转换

- 摘要必须有合法 UUID、名称和 `owner/project` 路径。缺失可选作者信息以“未知作者”显示；缺失计数为 0；无效图片 URL 省略。
- `//` 图片地址补 HTTPS。附件 `/oshwhub/...` 路径对应 `https://image.lceda.cn`；已通过公开 Range GET 验证 ZIP 文件头。
- 正文按 HTML 解析成标题、段落、代码、图片和链接；脚本、样式、iframe、对象和嵌入内容丢弃。正文链接仅接受无凭据的 http/https URL。树深度、正文大小和内容块数量受限；每个文本节点只抽取一次，避免嵌套 div 重复。
- `proImages` 是板版本、PCB ID、缩略图和 BOM 元数据，不是矢量 PCB。本轮真实 PCB 由独立 EDA 历史链路提供。
- BOM 为官网嵌套 JSON 字符串；严格检查数组与数量，损坏数据报告 `INVALID_BOM`。板上 BOM 缺失可以为空，不把解析失败伪装成空表。
- 官网列表 `members` 是字符串数组，搜索/详情同字段为对象数组，桥摘要不保留该不稳定字段。标签仅取已有名称，不把项目分类 UUID 展示为名称。

## 实测边界

2026-10-23 00:13–00:25 Asia/Shanghai，无 Cookie/Authorization：浏览、第二页、最新/点赞、正式搜索及第二页、搜索最新、详情和 proImages 均 HTTP 200 且业务 code=0。工程 `90602f2f840d4245a97479e90a31d17b` 有三个板版本。

- ESP32 正式搜索 total=2451；GET 列表关键词 total=1594，证明二者不同。
- 不存在工程返回 HTTP 200、success=false、code=104001；必须检查业务 envelope。
- 官方推荐合集请求 pageSize=6 时曾返回十行、六个唯一 UUID，后四行重复。通用 UUID 去重覆盖该实际异常。
- PCB 缩略图 `https://image-pro.lceda.cn/projects/thumbs/0bd23fe834034e658f420214463c16f8.webp` 返回 217236 字节，RIFF/WEBP 魔数。
- 未复现 HTTP 418，前期保存的 HTML 是 CloudWAF 拦截页。连接超时与站点限流分别报告。

`tests/fixtures/api-*` 是字段与形状取自实测的最小合成夹具，正文、异常链接和重复条目为安全测试构造；应用不会将它们作为离线实时结果。完整原始调查记录位于本机 `/tmp/oshwhub-api-research`，不包含登录态。

离线验证：`cargo test -p oshwhub-core`。在线验证独立且默认忽略：`cargo test -p oshwhub-core --test api_contract live_public_feed_search_and_project -- --ignored --nocapture`。

未验证分类过滤、最大页数、限流阈值、标准版 PCB 和三维模型。官网接口未提供稳定 SDK 承诺，字段变化会通过契约错误显式暴露。
