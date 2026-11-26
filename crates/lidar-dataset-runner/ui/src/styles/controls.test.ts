import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (name: string) => readFileSync(`src/styles/${name}`, "utf8");
const tokens = read("tokens.css");
const base = read("base.css");

describe("control theming", () => {
  it("declares a color scheme so native widgets follow the theme", () => {
    expect(tokens).toMatch(/color-scheme:\s*dark/);
  });

  it("stops the browser painting form fields with its own light theme", () => {
    expect(base).toMatch(/appearance:\s*none/);
  });

  it("paints selects and their options with theme colours", () => {
    expect(base).toMatch(/select\s*{[^}]*background-color:/);
    expect(base).toMatch(/option\s*{[^}]*background-color:/);
  });
});
