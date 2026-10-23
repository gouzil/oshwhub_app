export type Sort = "recommend" | "new" | "like";

export interface ProjectSummary {
  id: string;
  name: string;
  path: string;
  description: string;
  cover: string | null;
  author: string;
  authorAvatar: string | null;
  likes: number;
  views: number;
  tags: string[];
  updatedAt: string | null;
  url: string;
}
export interface FeedPage {
  items: ProjectSummary[];
  page: number;
  total: number;
  hasMore: boolean;
}
export interface ContentBlock {
  kind: string;
  text: string;
  url: string | null;
}
export interface BomItem {
  name: string;
  designator: string;
  footprint: string;
  quantity: number;
  partNumber: string;
  manufacturer: string;
  supplier: string;
  supplierPart: string;
}
export interface Board {
  id: string;
  name: string;
  pcbId: string | null;
  thumbnail: string | null;
  bom: BomItem[];
}
export interface ProjectDetail {
  summary: ProjectSummary;
  content: ContentBlock[];
  boards: Board[];
  attachments: { name: string; url: string }[];
}
export interface Bounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}
export interface SceneLayer {
  id: number;
  name: string;
  color: string;
  visible: boolean;
  order: number;
}
export interface DrawBatch {
  layer: number;
  path: string;
  fill: boolean;
  width: number;
  bounds: Bounds;
  color?: string | null;
}
export interface TextLabel {
  layer: number;
  x: number;
  y: number;
  text: string;
  size: number;
  rotation: number;
  align?: "left" | "center" | "right";
}
export interface Scene {
  id: string;
  name: string;
  revision: string;
  bounds: Bounds;
  layers: SceneLayer[];
  batches: DrawBatch[];
  texts: TextLabel[];
  objectCount: number;
  warnings: string[];
  metrics: {
    loadMs: number;
    parseMs: number;
    sceneMs: number;
    recordCount: number;
    historyCount: number;
    downloadBytes: number;
  };
}
export interface PickResult {
  id: string;
  kind: string;
  layer: number;
  net: string | null;
  properties: Record<string, string>;
  bounds: Bounds;
}
export type Command =
  | { command: "health" }
  | { command: "feed"; page: number; pageSize?: number; query?: string; sort?: Sort }
  | { command: "project"; id: string }
  | { command: "pcb"; projectId: string; documentId: string }
  | { command: "demo" }
  | {
      command: "pick";
      sceneId: string;
      x: number;
      y: number;
      tolerance: number;
      layers?: number[];
    };
export interface CoreError {
  code: string;
  message: string;
  requestId: string;
}
export type Envelope<T> =
  | { ok: true; data: T; requestId: string }
  | { ok: false; error: CoreError };
