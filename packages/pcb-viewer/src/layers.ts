import type { DrawBatch, Scene, SceneImage, SceneLayer, TextLabel } from "@oshwhub/bridge";
export type LayerPlan = {
  layer: SceneLayer;
  batches: DrawBatch[];
  texts: TextLabel[];
  images: SceneImage[];
};
export function planLayers(scene: Scene): LayerPlan[] {
  const layers: LayerPlan[] = scene.layers.map((layer) => ({
    layer,
    batches: [],
    texts: [],
    images: [],
  }));
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
  for (const image of scene.images ?? []) {
    const plan = byId.get(image.layer);
    if (!plan) throw new Error(`图片包含未声明的图层 ${image.layer}`);
    plan.images.push(image);
  }
  return layers.sort((a, b) => a.layer.order - b.layer.order);
}
