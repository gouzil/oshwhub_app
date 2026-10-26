import type { Bounds, Scene } from "@oshwhub/bridge";

export type NetKind = "pad" | "track" | "via" | "other";
export type NetSummary = {
  name: string;
  bounds: Bounds;
  primitiveCount: number;
};

export const DEFAULT_NET_COLORS = [
  "#ffd166",
  "#4cc9f0",
  "#ff6b8a",
  "#6ee7b7",
  "#c084fc",
  "#fb923c",
] as const;

export function netKind(kind: string | null | undefined): NetKind {
  if (kind === "PAD") return "pad";
  if (kind === "VIA") return "via";
  if (kind === "LINE" || kind === "ARC" || kind === "CARC" || kind === "TRACK") return "track";
  return "other";
}

export function isCopperLayer(layer: number): boolean {
  return layer === 1 || layer === 2 || layer === 12 || (layer >= 15 && layer <= 46);
}

function extendBounds(a: Bounds, b: Bounds): Bounds {
  return {
    minX: Math.min(a.minX, b.minX),
    minY: Math.min(a.minY, b.minY),
    maxX: Math.max(a.maxX, b.maxX),
    maxY: Math.max(a.maxY, b.maxY),
  };
}

export function collectNets(scene: Pick<Scene, "batches">): NetSummary[] {
  const byName = new Map<string, NetSummary>();
  for (const batch of scene.batches) {
    const name = batch.net?.trim();
    if (!name || !isCopperLayer(batch.layer)) continue;
    const previous = byName.get(name);
    if (previous) {
      previous.bounds = extendBounds(previous.bounds, batch.bounds);
      previous.primitiveCount += 1;
    } else {
      byName.set(name, {
        name,
        bounds: { ...batch.bounds },
        primitiveCount: 1,
      });
    }
  }
  return [...byName.values()].sort((a, b) => a.name.localeCompare(b.name, "zh-CN"));
}

export function defaultNetColor(name: string): string {
  let hash = 0;
  for (const character of name) hash = (hash * 31 + (character.codePointAt(0) ?? 0)) >>> 0;
  return DEFAULT_NET_COLORS[hash % DEFAULT_NET_COLORS.length];
}

export function nextNetColor(current: string): string {
  const index = DEFAULT_NET_COLORS.indexOf(current as (typeof DEFAULT_NET_COLORS)[number]);
  return DEFAULT_NET_COLORS[(index + 1 + DEFAULT_NET_COLORS.length) % DEFAULT_NET_COLORS.length];
}
