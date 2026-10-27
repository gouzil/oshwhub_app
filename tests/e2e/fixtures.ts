import { join } from "node:path";
import { test as base, expect } from "@playwright/test";
import type {
  BomItem,
  Command,
  ContentBlock,
  FeedPage,
  PackedMesh,
  PickResult,
  ProjectDetail,
  ProjectSummary,
  Scene,
} from "../../packages/bridge/src/types";

export const unsafeMarkup = "<script>window.__e2eUnsafe = true</script><b>原样正文</b>";

const summary = (index: number): ProjectSummary => ({
  id: `self-made-${index}`,
  name: `自制开源控制板 ${String(index).padStart(2, "0")}`,
  path: `offline/self-made-${index}`,
  description: "完全自制的离线测试项目，用于验证公开浏览与 PCB 交互。",
  cover: null,
  author: "离线创作者",
  authorAvatar: null,
  likes: 128 + index,
  views: 2048,
  tags: ["自制夹具", "中文"],
  updatedAt: "2026-10-22T08:00:00Z",
  url: `https://example.com/self-made-${index}`,
});

const project = (id: string): ProjectDetail => ({
  summary: summary(Number(id.replace("self-made-", ""))),
  content: [
    { kind: "heading", text: "自制项目说明", url: null },
    { kind: "text", text: unsafeMarkup, url: null },
    { kind: "code", text: "const voltage = 3.3;", url: null },
    { kind: "image", text: "本地自制说明图片", url: "http://127.0.0.1:5273/__e2e__/article.png" },
    { kind: "link", text: "无效脚本链接", url: "javascript:window.__e2eUnsafe=true" },
  ],
  boards: [
    {
      id: "board-main",
      name: "自制主板",
      pcbId: "pcb-main",
      schematicId: "schematic-main",
      thumbnail: null,
      bom: [
        {
          name: "中文微控制器",
          designator: "U1",
          footprint: "QFN-32",
          quantity: 1,
          partNumber: "SELF-MCU-001",
          manufacturer: "自制半导体",
          supplier: "测试供应商",
          supplierPart: "TEST-001",
        },
        {
          name: "电阻",
          designator: "R1, R2",
          footprint: "0603",
          quantity: 2,
          partNumber: "",
          manufacturer: "",
          supplier: "",
          supplierPart: "",
        },
      ],
    },
    {
      id: "board-empty",
      name: "自制扩展板",
      pcbId: null,
      schematicId: null,
      thumbnail: null,
      bom: [],
    },
  ],
  attachments: [
    { name: "自制说明书", url: "https://example.com/manual.pdf" },
    { name: "无效附件", url: "data:text/html,unsafe" },
  ],
});

export const scene: Scene = {
  id: "self-made-scene",
  name: "自制中文测试 PCB",
  revision: "offline-1",
  bounds: { minX: 0, minY: 0, maxX: 120, maxY: 80 },
  layers: [
    { id: 90, name: "技术参考层 A", color: "#718096", visible: false, order: -2 },
    { id: 91, name: "技术参考层 B", color: "#718096", visible: false, order: -1 },
    { id: 1, name: "顶层铜", color: "#ef6b62", visible: true, order: 1 },
    { id: 2, name: "底层铜", color: "#3bc98c", visible: true, order: 2 },
    { id: 3, name: "中文丝印", color: "#f8f6e8", visible: true, order: 3 },
    { id: 47, name: "孔洞", color: "#0b121a", visible: true, order: 47 },
  ],
  batches: [
    {
      layer: 1,
      path: "M10 10H110V70H10Z M36 26V44H54V26Z M20 50V62H40V50Z",
      fill: true,
      width: 0,
      bounds: { minX: 10, minY: 10, maxX: 110, maxY: 70 },
    },
    {
      layer: 2,
      path: "M20 50H100V62H20Z",
      fill: true,
      width: 0,
      bounds: { minX: 20, minY: 50, maxX: 100, maxY: 62 },
    },
    {
      layer: 47,
      path: "M87 30H93V38H87Z",
      fill: true,
      width: 0,
      bounds: { minX: 87, minY: 30, maxX: 93, maxY: 38 },
    },
  ],
  texts: [{ layer: 3, x: 20, y: 23, text: "自制中文", size: 8, rotation: 0, align: "left" }],
  objectCount: 5,
  warnings: [],
  metrics: { loadMs: 1, parseMs: 2, sceneMs: 3, recordCount: 5, historyCount: 1, downloadBytes: 0 },
};

export const schematicScene: Scene = {
  id: "self-made-schematic-scene",
  name: "自制控制板原理图",
  revision: "offline-sch-1",
  bounds: { minX: 0, minY: 0, maxX: 160, maxY: 100 },
  layers: [
    { id: 1, name: "连线", color: "#5ca9e6", visible: true, order: 100 },
    { id: 3, name: "符号", color: "#e8d38b", visible: true, order: 300 },
    { id: 4, name: "引脚", color: "#e38c8c", visible: true, order: 400 },
    { id: 5, name: "文本", color: "#ffffff", visible: true, order: 500 },
  ],
  batches: [
    {
      layer: 1,
      path: "M20 50H80 M80 50V30",
      fill: false,
      width: 1,
      bounds: { minX: 20, minY: 30, maxX: 80, maxY: 50 },
    },
    {
      layer: 3,
      path: "M70 40H100V60H70Z",
      fill: false,
      width: 1,
      bounds: { minX: 70, minY: 40, maxX: 100, maxY: 60 },
    },
    {
      layer: 4,
      path: "M100 50H125",
      fill: false,
      width: 1,
      bounds: { minX: 100, minY: 50, maxX: 125, maxY: 50 },
    },
  ],
  texts: [{ layer: 5, x: 28, y: 25, text: "POWER", size: 8, rotation: 0, align: "left" }],
  objectCount: 4,
  warnings: ["离线原理图夹具"],
  metrics: { loadMs: 1, parseMs: 2, sceneMs: 3, recordCount: 8, historyCount: 1, downloadBytes: 0 },
};

// Original box geometry, independent of the Rust board/model compiler.
export const boxMesh = (min: number[], max: number[], color: number[]): PackedMesh => {
  const points = [
    [min[0], min[1], min[2]],
    [max[0], min[1], min[2]],
    [max[0], max[1], min[2]],
    [min[0], max[1], min[2]],
    [min[0], min[1], max[2]],
    [max[0], min[1], max[2]],
    [max[0], max[1], max[2]],
    [min[0], max[1], max[2]],
  ];
  const faces = [
    { indices: [0, 3, 2, 1], normal: [0, 0, -1] },
    { indices: [4, 5, 6, 7], normal: [0, 0, 1] },
    { indices: [0, 1, 5, 4], normal: [0, -1, 0] },
    { indices: [3, 7, 6, 2], normal: [0, 1, 0] },
    { indices: [1, 2, 6, 5], normal: [1, 0, 0] },
    { indices: [0, 4, 7, 3], normal: [-1, 0, 0] },
  ];
  const data = Float32Array.from(
    faces.flatMap((face) =>
      [0, 1, 2, 0, 2, 3].flatMap((i) => [...points[face.indices[i]], ...face.normal, ...color]),
    ),
  );
  return {
    data: Buffer.from(data.buffer).toString("base64"),
    vertexCount: data.length / 9,
    indices: Buffer.from(
      Uint32Array.from({ length: data.length / 9 }, (_, i) => i).buffer,
    ).toString("base64"),
    indexCount: data.length / 9,
    bounds: { min, max },
  };
};

export const test = base.extend<{
  bridge: {
    requests: Command[];
    failNextFeed: boolean;
    endlessFeed: boolean;
    feedGate?: Promise<void>;
    projectGate?: Promise<void>;
    projectContent?: ContentBlock[];
    projectCover?: string;
    projectBom?: BomItem[];
    pickResult?: PickResult | null;
    modelGate?: Promise<void>;
    failModels?: boolean;
  };
}>({
  bridge: async ({ page }, use, testInfo) => {
    const bridge: {
      requests: Command[];
      failNextFeed: boolean;
      endlessFeed: boolean;
      feedGate?: Promise<void>;
      projectGate?: Promise<void>;
      projectContent?: ContentBlock[];
      projectCover?: string;
      projectBom?: BomItem[];
      pickResult?: PickResult | null;
      modelGate?: Promise<void>;
      failModels?: boolean;
    } = { requests: [], failNextFeed: false, endlessFeed: false };
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
    page.on("console", (message) => {
      if (message.type() !== "error") return;
      // Chromium reports the deliberately injected HTTP failure as a resource error.
      if (/Failed to load resource:.*status of 418/.test(message.text())) return;
      errors.push(`console.error: ${message.text()}`);
    });
    await page.route("http://127.0.0.1:5273/__e2e__/article.png", (route) =>
      route.fulfill({
        contentType: "image/png",
        path: join(testInfo.project.testDir, "assets", "article.png"),
      }),
    );
    await page.route("http://127.0.0.1:4318/bridge", async (route) => {
      const command = route.request().postDataJSON() as Command;
      bridge.requests.push(command);
      if (command.command === "feed" && bridge.failNextFeed) {
        bridge.failNextFeed = false;
        await route.fulfill({ status: 418, body: "deliberate offline test failure" });
        return;
      }
      let data: unknown;
      switch (command.command) {
        case "feed": {
          await bridge.feedGate;
          const response: FeedPage = {
            items: Array.from({ length: 20 }, (_, i) => summary((command.page - 1) * 20 + i + 1)),
            page: command.page,
            total: bridge.endlessFeed ? 100_000 : 40,
            hasMore: bridge.endlessFeed || command.page < 2,
          };
          data = response;
          break;
        }
        case "project": {
          await bridge.projectGate;
          const detail = project(command.id);
          if (bridge.projectContent) detail.content = bridge.projectContent;
          if (bridge.projectCover) detail.summary.cover = bridge.projectCover;
          if (bridge.projectBom) detail.boards[0].bom = bridge.projectBom;
          data = detail;
          break;
        }
        case "searchBom": {
          const query = command.query.trim().toLowerCase();
          data = command.items.flatMap((item, index) =>
            Object.values(item).some((value) => String(value).toLowerCase().includes(query))
              ? [index]
              : [],
          );
          break;
        }
        case "pcb":
          data = scene;
          break;
        case "schematic":
          data = schematicScene;
          break;
        case "demo":
          data = scene;
          break;
        case "pick": {
          if (command.sceneId === schematicScene.id) {
            data = {
              id: "self-symbol",
              kind: "COMPONENT",
              layer: 3,
              net: null,
              properties: { Designator: "U1", Value: "自制 MCU" },
              bounds: { minX: 70, minY: 40, maxX: 100, maxY: 60 },
            };
            break;
          }
          const top = command.activeLayer !== 2 && (command.layers?.includes(1) ?? true);
          const result: PickResult = {
            id: top ? "self-top-pad" : "self-bottom-pad",
            kind: "焊盘",
            layer: top ? 1 : 2,
            net: "GND",
            properties: { 名称: "中文焊盘", 编号: "1" },
            bounds: { minX: 56, minY: 36, maxX: 64, maxY: 44 },
          };
          data = bridge.pickResult === undefined ? result : bridge.pickResult;
          break;
        }
        case "pcb3d":
          data = {
            board: boxMesh([0, -80, 0], [120, 0, 4], [0.06, 0.36, 0.23]),
            modelKeys: ["self-model"],
            componentCount: 2,
            boundCount: 1,
            thickness: 4,
            warnings: ["自制离线三维夹具"],
          };
          break;
        case "model3d":
          await bridge.modelGate;
          if (bridge.failModels) {
            await route.fulfill({
              contentType: "application/json",
              json: {
                ok: false,
                error: { code: "BLOCKED", message: "自制网络失败", requestId: "model-test" },
              },
            });
            return;
          }
          data = {
            key: command.key,
            mesh: boxMesh([40, -55, 4], [65, -30, 22], [0.72, 0.55, 0.25]),
            instances: [{ title: "U1", matrix: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] }],
          };
          break;
        default:
          data = { version: "offline-e2e" };
      }
      await route.fulfill({
        contentType: "application/json",
        json: { ok: true, data, requestId: `e2e-${bridge.requests.length}` },
      });
    });
    await use(bridge);
    expect(errors, "No unexpected browser runtime errors").toEqual([]);
  },
});

export { expect };
