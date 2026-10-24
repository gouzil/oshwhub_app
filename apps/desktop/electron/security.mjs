export const MAX_REQUEST_BYTES = 1024 * 1024;

export function validateInput(input) {
  if (typeof input !== "string" || Buffer.byteLength(input, "utf8") > MAX_REQUEST_BYTES) {
    throw new Error("Invalid core request: maximum size is 1 MiB");
  }
  return input;
}

export function externalUrl(input) {
  if (typeof input !== "string" || input.length > 8192 || /[\s\p{Cc}]/u.test(input)) {
    throw new Error("Invalid external URL");
  }
  const url = new URL(input);
  if (!["https:", "http:"].includes(url.protocol) || url.username || url.password) {
    throw new Error("External links must use HTTP or HTTPS without credentials");
  }
  return url.href;
}

export function assertTrustedSender(event, window, rendererUrl) {
  if (
    !window ||
    window.isDestroyed() ||
    event.sender !== window.webContents ||
    event.senderFrame !== window.webContents.mainFrame
  ) {
    throw new Error("Untrusted desktop IPC sender");
  }
  const actual = new URL(event.senderFrame.url);
  const expected = new URL(rendererUrl);
  actual.hash = "";
  expected.hash = "";
  if (actual.href !== expected.href) throw new Error("Untrusted desktop IPC location");
}
