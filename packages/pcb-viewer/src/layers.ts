import type { DrawBatch, Scene, SceneLayer, TextLabel } from "@oshwhub/bridge";
export type LayerPlan = { layer: SceneLayer; batches: DrawBatch[]; texts: TextLabel[] };
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
