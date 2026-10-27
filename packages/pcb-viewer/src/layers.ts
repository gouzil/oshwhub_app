import type { DrawBatch, Scene, SceneLayer, TextLabel } from "@oshwhub/bridge";
export type LayerPlan = { layer: SceneLayer; batches: DrawBatch[]; texts: TextLabel[] };
export type BoardSide = 1 | 2;

export function pcbDisplayLayers(layers: SceneLayer[]): SceneLayer[] {
  return layers.map((layer) => ({
    ...layer,
    color: layer.id === 1 ? "#ff0000" : layer.id === 2 ? "#0000ff" : layer.color,
  }));
}

export function layerSide(id: number): BoardSide | null {
  return id >= 1 && id <= 10 ? (id % 2 ? 1 : 2) : null;
}

export function layerBrightness(id: number, activeLayer?: BoardSide): number {
  return activeLayer === undefined || layerSide(id) === activeLayer || id === 11 || id === 47
    ? 1
    : 0.4;
}

export function compareLayers(a: SceneLayer, b: SceneLayer, activeLayer?: BoardSide): number {
  if (activeLayer === undefined) return a.order - b.order;
  // Board edges and drill masks stay above both faces.
  const rank = (id: number) =>
    id === 47 ? 3 : id === 11 ? 2 : layerSide(id) === activeLayer ? 1 : 0;
  return rank(a.id) - rank(b.id) || a.order - b.order;
}

export function planLayers(scene: Scene): LayerPlan[] {
  const layers: LayerPlan[] = scene.layers.map((layer) => ({ layer, batches: [], texts: [] }));
  const byId = new Map(layers.map((plan) => [plan.layer.id, plan]));
  for (const batch of scene.batches) {
    const plan = byId.get(batch.layer);
    if (!plan) throw new Error(`场景包含未声明的图层 ${batch.layer}`);
    plan.batches.push(batch);
  }
  for (const text of scene.texts) {
    const plan = byId.get(text.layer);
    if (!plan) throw new Error(`文字包含未声明的图层 ${text.layer}`);
    plan.texts.push(text);
  }
  return layers.sort((a, b) => a.layer.order - b.layer.order);
}
