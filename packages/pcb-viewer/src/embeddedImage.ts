import { Skia } from "@shopify/react-native-skia";

export async function decodeSvg(content: string) {
  const data = Skia.Data.fromBase64(content.slice(content.indexOf(",") + 1));
  try {
    const svg = Skia.SVG.MakeFromData(data);
    if (!svg) throw new Error("原理图 SVG 图片无法解码。");
    return svg;
  } finally {
    data.dispose();
  }
}
