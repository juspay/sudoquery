import { defineConfig } from "tsup";
import { readFileSync, writeFileSync } from "node:fs";

const SHOPIFY_BUNDLE = "dist/shopify/index.js";

export default defineConfig({
  entry: ["shopify/index.js"],
  format: ["iife"],
  platform: "browser",
  globalName: "ShopifySudoQueryPixel",
  outDir: "dist/shopify",
  clean: true,
  sourcemap: false,
  dts: false,
  outExtension() {
    return {
      js: ".js",
    };
  },
  async onSuccess() {
    const source = readFileSync(SHOPIFY_BUNDLE, "utf8");
    writeFileSync(SHOPIFY_BUNDLE, source.replace(/^"use strict";\r?\n/, ""));
  },
});
