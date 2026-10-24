import { realpath } from "node:fs/promises";
import path from "node:path";

export async function assetPath(input, directory) {
  const url = new URL(input);
  if (
    url.protocol !== "oshwhub:" ||
    url.hostname !== "app" ||
    url.port ||
    url.username ||
    url.password
  ) {
    throw new Error("Unknown application asset origin");
  }
  // Inspect the original URL too: URL parsing normalizes encoded dot segments.
  const originalPath = decodeURIComponent(
    input
      .slice(input.indexOf("://") + 3)
      .replace(/^[^/]*\//u, "/")
      .split(/[?#]/u)[0],
  );
  const pathname = decodeURIComponent(url.pathname);
  if (
    pathname.includes("\\") ||
    pathname.includes("\0") ||
    originalPath.split("/").some((segment) => segment === "..")
  ) {
    throw new Error("Invalid application asset path");
  }
  const root = await realpath(directory);
  const file = await realpath(
    path.resolve(root, `.${pathname === "/" ? "/index.html" : pathname}`),
  );
  const relative = path.relative(root, file);
  if (
    !relative ||
    relative.startsWith(`..${path.sep}`) ||
    relative === ".." ||
    path.isAbsolute(relative)
  ) {
    throw new Error("Application asset is outside the bundle");
  }
  return file;
}
