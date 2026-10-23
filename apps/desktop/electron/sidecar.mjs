import { validateInput } from "./security.mjs";

export class Sidecar {
  #child;
  #pending = new Map();
  #parts = [];
  #bytes = 0;
  #sequence = 0;
  #failure;
  #timeoutMs;
  #maxPending;
  #maxResponseBytes;

  constructor(
    child,
    { timeoutMs = 240_000, maxPending = 32, maxResponseBytes = 128 * 1024 * 1024 } = {},
  ) {
    this.#child = child;
    this.#timeoutMs = timeoutMs;
    this.#maxPending = maxPending;
    this.#maxResponseBytes = maxResponseBytes;
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => this.#receive(chunk));
    child.stdout.on("error", () => this.#fail(new Error("Core sidecar output failed")));
    child.stdout.on("end", () => this.#fail(new Error("Core sidecar output closed")));
    child.stdin.on("error", () => this.#fail(new Error("Core sidecar input failed")));
    child.stderr.on("error", () => this.#fail(new Error("Core sidecar diagnostics stream failed")));
    child.stderr.resume();
    child.on("error", () => this.#fail(new Error("Core sidecar failed to start")));
    child.on("exit", () => this.#fail(new Error("Core sidecar exited")));
  }

  dispatch(input) {
    try {
      validateInput(input);
    } catch (error) {
      return Promise.reject(error);
    }
    if (this.#failure) return Promise.reject(this.#failure);
    if (this.#pending.size >= this.#maxPending)
      return Promise.reject(new Error("Core sidecar is busy"));
    const id = String(++this.#sequence);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => this.#fail(new Error("Core sidecar timed out")),
        this.#timeoutMs,
      );
      this.#pending.set(id, { resolve, reject, timer });
      try {
        this.#child.stdin.write(`${JSON.stringify({ id, input })}\n`, (error) => {
          if (error) this.#fail(new Error("Core sidecar input failed"));
        });
      } catch {
        this.#fail(new Error("Core sidecar input failed"));
      }
    });
  }

  #receive(chunk) {
    if (this.#failure) return;
    let remaining = chunk;
    while (remaining.length) {
      const newline = remaining.indexOf("\n");
      const part = newline === -1 ? remaining : remaining.slice(0, newline);
      this.#bytes += Buffer.byteLength(part, "utf8");
      if (this.#bytes > this.#maxResponseBytes) {
        this.#fail(new Error("Core sidecar response exceeds size limit"));
        return;
      }
      this.#parts.push(part);
      if (newline === -1) return;
      const line = this.#parts.join("");
      this.#parts = [];
      this.#bytes = 0;
      remaining = remaining.slice(newline + 1);
      try {
        const response = JSON.parse(line);
        if (typeof response.id !== "string" || typeof response.output !== "string")
          throw new Error();
        const pending = this.#pending.get(response.id);
        if (!pending) throw new Error();
        clearTimeout(pending.timer);
        this.#pending.delete(response.id);
        pending.resolve(response.output);
      } catch {
        this.#fail(new Error("Core sidecar returned an invalid response"));
        return;
      }
    }
  }

  #fail(error) {
    if (this.#failure) return;
    this.#failure = error;
    this.#parts = [];
    this.#bytes = 0;
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.#pending.clear();
    this.#child.stdin.destroy();
    this.#child.kill();
  }

  close() {
    this.#fail(new Error("Core sidecar closed"));
  }
}
