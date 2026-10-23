import type { PickResult } from "@oshwhub/bridge";

const kinds: Record<string, string> = {
  COMPONENT: "元件",
  PAD: "焊盘",
  VIA: "过孔",
  LINE: "走线",
  ARC: "圆弧",
  CARC: "圆弧",
  POLY: "轮廓",
  FILL: "填充",
  REGION: "区域",
  POUR: "覆铜区域",
  POURED: "覆铜",
  TEARDROP: "泪滴",
  FPC_FILL: "补强区域",
  STRING: "文字",
  TEXT: "文字",
  ATTR: "元件标注",
  IMAGE: "图像",
  DIMENSION: "尺寸标注",
};
const labels: Record<string, string> = {
  Name: "名称",
  Value: "参数",
  DeviceName: "器件",
  FootprintName: "封装",
  ManufacturerPart: "型号",
  MPN: "型号",
  Manufacturer: "制造商",
  PadNumber: "焊盘编号",
  Text: "内容",
};
const internal = new Set([
  "recordId",
  "ticket",
  "componentId",
  "Device",
  "Footprint",
  "Channel ID",
  "Unique ID",
]);

export function describeSelection(selection: PickResult) {
  const properties = selection.properties;
  const designator = (properties.Designator || properties.位号 || "").trim();
  const name = (properties.Name || properties.名称 || "").trim();
  const kind = Object.hasOwn(kinds, selection.kind)
    ? kinds[selection.kind]
    : /\p{Script=Han}/u.test(selection.kind)
      ? selection.kind
      : "图元";
  const pad = kind === "焊盘" ? (properties.PadNumber || properties.编号 || "").trim() : "";
  const title = pad
    ? `${designator ? `${designator} · ` : ""}焊盘 ${pad}`
    : designator || name || kind;
  const fields: [string, string][] = selection.net ? [["网络", selection.net]] : [];
  for (const [key, raw] of Object.entries(properties)) {
    const value = raw.trim();
    if (
      !value ||
      internal.has(key) ||
      key === "Designator" ||
      key === "位号" ||
      ((key === "Name" || key === "名称") && value === title) ||
      (pad && (key === "PadNumber" || key === "编号"))
    )
      continue;
    fields.push([Object.hasOwn(labels, key) ? labels[key] : key, value]);
  }
  return { title, kind, fields };
}
