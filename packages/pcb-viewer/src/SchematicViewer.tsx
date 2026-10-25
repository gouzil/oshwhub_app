import type { PickResult, Scene } from "@oshwhub/bridge";
import { PcbViewer } from "./PcbViewer";

export function SchematicViewer({
  scene,
  onPick,
}: {
  scene: Scene;
  onPick?: (selection: PickResult | null) => void;
}) {
  return <PcbViewer scene={scene} onPick={onPick} variant="schematic" />;
}
