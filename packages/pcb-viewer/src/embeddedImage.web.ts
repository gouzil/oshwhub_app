import { Skia } from "@shopify/react-native-skia";

export async function decodeSvg(content: string) {
  const image = new Image();
  image.src = content;
  await image.decode();
  // CanvasKit's SVG wrapper draws before its HTML image has loaded; decode explicitly.
  const canvas = document.createElement("canvas");
  // shortcut: cache SVGs at 4×, capped at 4096px; use vector decoding for deeper logo zoom.
  const scale = Math.min(4, 4096 / Math.max(image.naturalWidth, image.naturalHeight, 1));
  canvas.width = Math.max(1, Math.ceil(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.ceil(image.naturalHeight * scale));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("无法解码原理图 SVG 图片。");
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  return Skia.Image.MakeImageFromNativeBuffer(canvas);
}
