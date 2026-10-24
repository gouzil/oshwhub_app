import { describe, expect, it } from "vitest";
import { cn } from "./utils";

describe("shared component class overrides", () => {
  it("lets callers replace spacing and theme colors while preserving interaction variants", () => {
    expect(cn("px-4 bg-primary hover:bg-primary/90", "px-6 bg-secondary")).toBe(
      "hover:bg-primary/90 px-6 bg-secondary",
    );
  });

  it("accepts conditional and optional classes used by disabled controls", () => {
    expect(cn("min-h-11", false, undefined, { "opacity-50": true, hidden: false })).toBe(
      "min-h-11 opacity-50",
    );
  });
});
