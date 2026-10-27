import { inflateSync } from "node:zlib";

// Decode Chromium's 8-bit RGB/RGBA screenshots so rendering checks inspect actual pixels.
export function canvasColors(png: Buffer) {
  const width = png.readUInt32BE(16);
  const height = png.readUInt32BE(20);
  const channels = png[25] === 6 ? 4 : png[25] === 2 ? 3 : 0;
  if (png[24] !== 8 || !channels) throw new Error("Unsupported screenshot PNG format");
  const compressed: Buffer[] = [];
  for (let offset = 8; offset < png.length; ) {
    const length = png.readUInt32BE(offset);
    if (png.toString("ascii", offset + 4, offset + 8) === "IDAT")
      compressed.push(png.subarray(offset + 8, offset + 8 + length));
    offset += length + 12;
  }
  const raw = inflateSync(Buffer.concat(compressed));
  const stride = width * channels;
  const pixels = Buffer.alloc(stride * height);
  const paeth = (left: number, up: number, diagonal: number) => {
    const prediction = left + up - diagonal;
    const dl = Math.abs(prediction - left);
    const du = Math.abs(prediction - up);
    const dd = Math.abs(prediction - diagonal);
    return dl <= du && dl <= dd ? left : du <= dd ? up : diagonal;
  };
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    for (let x = 0; x < stride; x++) {
      const i = y * stride + x;
      const left = x >= channels ? pixels[i - channels] : 0;
      const up = y ? pixels[i - stride] : 0;
      const diagonal = y && x >= channels ? pixels[i - stride - channels] : 0;
      const predictor =
        filter === 0
          ? 0
          : filter === 1
            ? left
            : filter === 2
              ? up
              : filter === 3
                ? Math.floor((left + up) / 2)
                : filter === 4
                  ? paeth(left, up, diagonal)
                  : NaN;
      if (!Number.isFinite(predictor)) throw new Error("Unsupported screenshot PNG filter");
      pixels[i] = (raw[y * (stride + 1) + x + 1] + predictor) & 255;
    }
  }
  const counts = { copper: 0, bottom: 0, chinese: 0 };
  for (let i = 0; i < pixels.length; i += channels) {
    // The floating red/blue side legend is UI rather than board geometry.
    if (Math.floor(i / stride) < 80 && (i / channels) % width > width - 180) continue;
    const [red, green, blue] = pixels.subarray(i, i + 3);
    // Allow CanvasKit's display-color-space conversion while distinguishing all fixture layers.
    if (red > 60 && red > green * 2 && red > blue * 2) counts.copper++;
    if (blue > 60 && blue > red * 2 && blue > green * 2) counts.bottom++;
    if (red > 220 && green > 220 && blue > 190) counts.chinese++;
  }
  return counts;
}
