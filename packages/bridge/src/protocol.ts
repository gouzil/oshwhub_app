export class BridgeError extends Error {
  constructor(
    public code: string,
    message: string,
    public requestId: string,
  ) {
    super(message);
    this.name = "BridgeError";
  }
}

export function decode<T>(raw: unknown): T {
  let value: unknown = raw;
  if (typeof raw === "string") {
    try {
      value = JSON.parse(raw);
    } catch {
      throw invalidResponse();
    }
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) throw invalidResponse();
  const envelope = value as Record<string, unknown>;
  if (
    envelope.ok === true &&
    Object.hasOwn(envelope, "data") &&
    typeof envelope.requestId === "string"
  ) {
    return envelope.data as T;
  }
  if (envelope.ok === false && envelope.error && typeof envelope.error === "object") {
    const error = envelope.error as Record<string, unknown>;
    if (
      typeof error.code === "string" &&
      typeof error.message === "string" &&
      typeof error.requestId === "string"
    ) {
      throw new BridgeError(error.code, error.message, error.requestId);
    }
  }
  throw invalidResponse();
}

function invalidResponse(): BridgeError {
  return new BridgeError(
    "bridge_invalid_response",
    "Rust 核心响应格式无效，请更新应用或查看诊断日志",
    "bridge",
  );
}
