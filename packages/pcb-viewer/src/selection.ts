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
  WIRE: "连线",
  BUS: "总线",
  BUSENTRY: "总线入口",
  PIN: "引脚",
  RECT: "矩形",
  CIRCLE: "圆形",
  ELLIPSE: "椭圆",
  BEZIER: "贝塞尔曲线",
  JUNCTION: "连接点",
  TABLE: "表格",
  OBJ: "图像",
};
const labels: Record<string, string> = {
  Name: "名称",
  Value: "值",
  DeviceName: "器件",
  FootprintName: "封装",
  ManufacturerPart: "型号",
  MPN: "型号",
  Manufacturer: "制造商",
  PadNumber: "焊盘编号",
  Text: "内容",
  "Manufacturer Part": "制造商编号",
  Supplier: "供应商",
  "Supplier Part": "供应商编号",
  "Supplier Footprint": "供应商封装",
  Description: "描述",
  Datasheet: "数据手册",
  "LCSC Part Name": "商品名称",
  "JLCPCB Part Class": "元件库分类",
  "Add into BOM": "加入 BOM",
  "Convert to PCB": "转到 PCB",
  "PCB Layer": "PCB 层",
  Tolerance: "精度",
  "Power(Watts)": "功率",
  "Resistance(Ohms)": "阻值",
  Capacitance: "电容量",
  Voltage: "电压",
  "Operating Temperature Range": "工作温度",
  "Overload Voltage (Max)": "最大过载电压",
  "Temperature Coefficient": "温度系数",
  Type: "类型",
  PinName: "引脚名称",
  PinNumber: "引脚编号",
  recordId: "记录 ID",
  ticket: "记录版本",
  componentId: "元件 ID",
  Device: "器件 ID",
  Symbol: "符号 ID",
  Footprint: "封装 ID",
  "Channel ID": "通道 ID",
  "Group ID": "分组 ID",
  "Reuse Block": "复用模块",
  "Unique ID": "唯一 ID",
  "3D Model": "三维模型 ID",
  "3D Model Title": "三维模型",
  "3D Model Transform": "三维模型变换",
};
const internal = new Set([
  "recordId",
  "ticket",
  "componentId",
  "Device",
  "Symbol",
  "Footprint",
  "Channel ID",
  "Unique ID",
  "Group ID",
  "Reuse Block",
  "3D Model",
  "3D Model Title",
  "3D Model Transform",
]);
const priority = [
  "Value",
  "DeviceName",
  "FootprintName",
  "Manufacturer",
  "Manufacturer Part",
  "ManufacturerPart",
  "MPN",
  "Supplier",
  "Supplier Part",
  "Name",
  "Text",
  "PadNumber",
  "PinName",
  "PinNumber",
];
const clean = (value?: string) => (value?.trim() === "null" ? "" : (value?.trim() ?? ""));

export function describeSelection(selection: PickResult) {
  const properties = selection.properties;
  const designator = clean(properties.Designator) || clean(properties.位号);
  const name = clean(properties.Name) || clean(properties.名称);
  const kind =
    properties.componentId && designator && !["PAD", "PIN"].includes(selection.kind)
      ? "元件"
      : Object.hasOwn(kinds, selection.kind)
        ? kinds[selection.kind]
        : /\p{Script=Han}/u.test(selection.kind)
          ? selection.kind
          : "图元";
  const pad = kind === "焊盘" ? (properties.PadNumber || properties.编号 || "").trim() : "";
  const title = pad
    ? `${designator ? `${designator} · ` : ""}焊盘 ${pad}`
    : designator || name || kind;
  const fields: [string, string][] = selection.net ? [["网络", selection.net]] : [];
  const extraFields: [string, string][] = [];
  const technicalFields: [string, string][] = [
    ["对象 ID", selection.id],
    ["原始类型", selection.kind],
  ];
  for (const [key, raw] of Object.entries(properties)) {
    const value = clean(raw);
    if (!value) continue;
    const label = Object.hasOwn(labels, key) ? labels[key] : key;
    if (internal.has(key)) {
      technicalFields.push([label, value]);
      continue;
    }
    if (
      key === "Designator" ||
      key === "位号" ||
      ((key === "Name" || key === "名称") && [title, clean(properties.Value)].includes(value)) ||
      (pad && (key === "PadNumber" || key === "编号"))
    )
      continue;
    const displayed = ["Add into BOM", "Convert to PCB"].includes(key)
      ? value === "yes"
        ? "是"
        : value === "no"
          ? "否"
          : value
      : value;
    const group = Object.hasOwn(labels, key) && !priority.includes(key) ? extraFields : fields;
    if (!group.some(([name, text]) => name === label && text === displayed))
      group.push([label, displayed]);
  }
  const rank = (label: string) => {
    const index = priority.findIndex((key) => labels[key] === label);
    return index < 0 ? priority.length : index;
  };
  fields.sort(([a], [b]) => (a === "网络" ? -1 : rank(a)) - (b === "网络" ? -1 : rank(b)));
  return { title, kind, fields, extraFields, technicalFields };
}
