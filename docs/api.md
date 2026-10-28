# 公开 API 契约

本轮只查询公开数据，不发送登录态、账号写入、浏览历史或计数上报。Rust 模型是桥协议的 camelCase JSON；官网 snake_case 字段在核心归一化。正文不会放进 WebView 执行。

## 请求与分页

- 浏览：`GET https://oshwhub.com/api/project`，参数 `page/pageSize/type/origin=all/projectTag=/keywords=/public=true`。
- 搜索：`POST https://oshwhub.com/api/search/data`，JSON 字段 `type=project, keywords, category="", sort, page, pageSize`。它是查询接口。GET 列表关键词筛选的覆盖范围不同，不能替代正式搜索。
- 页码从 1 开始，边界为 1–10000；每页 1–50；关键词去除首尾空白后最多 100 字符，拒绝控制字符。排序 `recommend/new/like` 分别为推荐、最新、点赞；搜索推荐映射空 sort。
- `page/pageSize/total/totalPage/lists` 来自官网响应。归一化按 UUID 保留第一次出现的记录；`hasMore` 按页元数据计算，不依赖去重后的行数。
- 详情：`GET /api/project/{id}`。专业版再读取 `/api/project/{id}/proImages`；标准版此端点返回 104006，因此只展示详情，不错误调用专业版预览接口。

## 本地物料搜索

`searchBom` 是本地桥命令，不请求官网：`{ command: "searchBom", items: BomItem[], query: string }` 返回匹配行的零基索引数组。搜索名称、位号、封装、数量、制造商型号、制造商、供应商与供应商料号；去除首尾空白后按不区分大小写的子串匹配，空关键词返回所有索引。关键词最多 100 字符且禁止控制字符，违反时返回 `INVALID_QUERY`；请求沿用 1 MiB 桥大小上限。展示层的“未提供”占位文案不参与匹配。

## HTTP 边界

只允许 HTTPS、443 端口、无用户名密码以及明确允许的 exact hostname。当前域名为 `oshwhub.com`、`pro.lceda.cn`、`modules.lceda.cn`、`image-pro.lceda.cn`、`image.lceda.cn`、`static.oshwhub.com`。不跟随重定向，避免允许域名跳转到任意目标。

连接超时 5 秒，总超时 30 秒；JSON 最多 8 MiB；二进制下载流式累计并执行调用者大小上限。JSON 必须返回 JSON MIME；HTML 拦截页、418、429 和业务错误都作为明确错误返回，不变成空页。PCB 缩略图实际可返回 `application/octet-stream`，二进制下载允许此 MIME。日志只保留方法、域名、状态和耗时，不记录完整 URL、查询、正文或密钥。

## 三维 PCB

- `{ command: "pcb3d", sceneId }` 从已有二维场景编译板体，返回 `board/thickness/componentCount/boundCount/modelKeys/warnings`；场景已淘汰返回 `scene_expired`，不在后台重下载。
- `{ command: "model3d", sceneId, key }` 仅接受当前场景实际绑定的模型 key，返回 `key/mesh/instances`。实例包含标题与列主序 4×4 装配矩阵，单位 mm；不会接收任意模型 URL。
- `PackedMesh` 的 `data` 是 base64 little-endian f32，交错位置/法线/RGB，每顶点 36 字节，附 `vertexCount` 和 `bounds.min/max`。`indices/indexCount` 为 base64 little-endian u32 三角索引；前端兼容旧核心的无索引网格。器件重复实例复用同一网格。
- 模型元数据使用 `GET https://pro.lceda.cn/api/v5/components/{seedUuid}` 的 `3d_model_uuid`，再下载 `https://modules.lceda.cn/3dmodel/{fileUuid}`。ID、所属场景、状态、大小均需校验；每个 OBJ 最多 12 MiB，内嵌材质以外的 MTL 和贴图不发请求。
- 当前在线元数据受 418/超时阻断，v5 响应形状与官方模型下载尚未完成本轮在线验收，详见 [3d.md](3d.md)。

## 原理图查看

- 专业版板卡优先使用 `schematic.documents[0].uuid`（实际图页）作为 `Board.schematicId`；`boards[].sch` / `schematic.uuid` 是分组 ID，仅在缺少图页列表时兼容旧响应。当前入口打开首个公开图页，尚无多页切换；非法或冲突 ID 返回明确错误。
- `{ command: "schematic", projectId, documentId }` 复用 PCB 的公开历史、AES-GCM/gzip 解码、大小上限和只读错误协议，返回同一 `Scene` 结构。统一 `{ command: "pick", sceneId, x, y, tolerance, layers }` 同时支持 PCB 与原理图场景。
- 原理图编译器读取 `SCH_PAGE` 与 `SYMBOL/PART`，支持线、总线、总线入口、引脚、文本/属性、矩形、多边形、圆、椭圆、圆弧、贝塞尔和连接点；元件旋转、镜像、平移后写入可绘制批次和可拾取属性。
- 未知类型、缺失符号/PART、损坏图元会聚合为 `Scene.warnings` 或稳定错误；不把空场景当成成功结果。支持合并单元格 TABLE 和嵌入 SVG/PNG/JPEG/WebP 的 OBJ；复杂层级复用仍未实现。

## 数据转换

- 摘要必须有合法 UUID、名称和 `owner/project` 路径。缺失可选作者信息以“未知作者”显示；缺失计数为 0；无效图片 URL 省略。
- `//` 图片地址补 HTTPS。附件 `/oshwhub/...` 路径对应 `https://image.lceda.cn`；已通过公开 Range GET 验证 ZIP 文件头。
- 正文按 HTML 解析成标题、段落、代码、图片和链接；脚本、样式、iframe、对象和嵌入内容丢弃。正文链接仅接受无凭据的 http/https URL。树深度、正文大小和内容块数量受限；每个文本节点只抽取一次，避免嵌套 div 重复。
- `proImages` 是板版本、PCB ID、缩略图和 BOM 元数据，不是矢量 PCB。本轮真实 PCB 由独立 EDA 历史链路提供。
- BOM 为官网嵌套 JSON 字符串；严格检查数组与数量，损坏数据报告 `INVALID_BOM`。板上 BOM 缺失可以为空，不把解析失败伪装成空表。
- BOM 的 `Name/Designator/Footprint/Quantity/Manufacturer Part/Manufacturer/Supplier/Supplier Part` 分别映射为 `name/designator/footprint/quantity/partNumber/manufacturer/supplier/supplierPart`。制造商与供应商单独保留，缺失的可选文本为 `""`；展示层统一标注“未提供”，不与其他字段合并。2026-10-23 对照 ESP32-S31 原网页，展开的 62 行 BOM 为纯文字，其公开 BOM 数据没有物料图片字段。
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
