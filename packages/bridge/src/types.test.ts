import { describe, expect, it } from "vitest";
import type { Command } from "./types";

describe("schematic bridge command", () => {
  it("accepts a read-only schematic scene request", () => {
    const command: Command = {
      command: "schematic",
      projectId: "project",
      documentId: "schematic",
    };
    expect(command).toMatchObject({ command: "schematic", documentId: "schematic" });
  });
});
