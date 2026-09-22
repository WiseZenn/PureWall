import test from "node:test";
import assert from "node:assert/strict";
import { readFile, readdir, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { isCaptureReady } from "./qa-screenshots.helpers.mjs";

const root = resolve(import.meta.dirname, "..");
const images = join(root, "docs/images");
const readmePath = join(root, "README.md");
const scriptPath = join(root, "scripts/qa-screenshots.mjs");
const views = ["library", "inspector", "displays"];
const themes = ["dark", "light"];
const expected = themes.flatMap((theme) => views.map((view) => `screenshot-${view}-${theme}.webp`)).sort();

function dimensions(buffer) {
  assert.equal(buffer.toString("ascii", 0, 4), "RIFF", "WebP must have a RIFF header");
  assert.equal(buffer.toString("ascii", 8, 12), "WEBP", "RIFF payload must be WebP");
  let offset = 12;
  while (offset + 8 <= buffer.length) {
    const type = buffer.toString("ascii", offset, offset + 4);
    const size = buffer.readUInt32LE(offset + 4);
    const data = offset + 8;
    if (type === "VP8X") return { width: 1 + buffer[data + 4] + (buffer[data + 5] << 8) + (buffer[data + 6] << 16), height: 1 + buffer[data + 7] + (buffer[data + 8] << 8) + (buffer[data + 9] << 16) };
    if (type === "VP8 " && buffer[data + 3] === 0x9d && buffer[data + 4] === 0x01 && buffer[data + 5] === 0x2a) return { width: buffer.readUInt16LE(data + 6) & 0x3fff, height: buffer.readUInt16LE(data + 8) & 0x3fff };
    if (type === "VP8L") return { width: 1 + ((buffer[data + 1] | (buffer[data + 2] << 8)) & 0x3fff), height: 1 + (((buffer[data + 2] >> 6) | (buffer[data + 3] << 2) | (buffer[data + 4] << 10)) & 0x3fff) };
    offset = data + size + (size % 2);
  }
  assert.fail("WebP dimensions chunk missing");
}

test("public screenshot manifest is exactly six themed WebPs", async () => {
  const entries = (await readdir(images)).filter((file) => file.startsWith("screenshot-")).sort();
  assert.deepEqual(entries, expected);
  assert.equal(entries.some((file) => file.includes("widget")), false);
  assert.equal(entries.some((file) => file.endsWith(".png")), false);
  for (const file of expected) assert.match(file, /^screenshot-(library|inspector|displays)-(dark|light)\.webp$/);
});

test("screenshot assets are large, nonblank, and 1600x1000", async () => {
  for (const file of expected) {
    const buffer = await readFile(join(images, file));
    assert.ok(buffer.length > 20 * 1024, `${file} must be larger than 20KB`);
    const size = dimensions(buffer);
    assert.deepEqual(size, { width: 1600, height: 1000 }, `${file} dimensions`);
    assert.ok(new Set(buffer.subarray(32)).size > 32, `${file} must not be blank`);
  }
  for (const view of views) {
    const dark = await readFile(join(images, `screenshot-${view}-dark.webp`));
    const light = await readFile(join(images, `screenshot-${view}-light.webp`));
    assert.notDeepEqual(dark, light, `${view} dark/light pair must differ`);
  }
});

test("README and capture script document the truthful synthetic render boundary", async () => {
  const readme = await readFile(readmePath, "utf8");
  const script = await readFile(scriptPath, "utf8");
  for (const file of expected) assert.match(readme, new RegExp(file.replaceAll(".", "\\.")));
  assert.match(readme, /dark/i);
  assert.match(readme, /light/i);
  assert.match(readme, /real Vue render/i);
  assert.match(readme, /mocked Tauri/i);
  assert.match(readme, /synthetic data/i);
  assert.match(readme, /not native WebView2/i);
  assert.match(readme, /not .*release evidence/i);
  assert.match(script, /Synthetic demo data/);
  assert.match(script, /purewall-theme/);
  assert.match(script, /purewall-workspace-mode/);
  assert.match(script, /data-theme/);
  assert.match(script, /Unknown mocked Tauri command/);
  assert.doesNotMatch(script, /<text\b/, "Synthetic SVG imageData must not contain decorative text");
  assert.doesNotMatch(script, new RegExp("PUREWALL /"), "Synthetic SVG must not contain per-image corner text");
});

test("capture-only badge and visual patch stay inside the capture boundary", async () => {
  const script = await readFile(scriptPath, "utf8");
  assert.match(script, /statusbar-shell/);
  assert.match(script, /host\.insertBefore\(badge/);
  assert.match(script, /bottom: \"44px\"/);
  assert.match(script, /224px minmax/);
  assert.match(script, /textOverflow === \"ellipsis\"/);
  assert.match(script, /route\.abort\(\"blockedbyclient\"\)/);
});

test("inspector capture clears card hover before taking the screenshot", async () => {
  const script = await readFile(scriptPath, "utf8");
  assert.match(script, /page\.mouse\.move\(1590, 990\)/);
  assert.match(script, /clearCardHoverAndAssert/);
  assert.match(script, /select-dot/);
  assert.match(script, /tile-overlay/);
  assert.match(script, /display === \"none\"/);
  assert.match(script, /Hover-only tile overlay or select control overlaps/);
});

test("readiness helper requires images, no loading placeholders, and stable geometry", () => {
  const ready = { imageCount: 4, imagesReady: true, loadingCount: 0, geometry: "stable" };
  assert.equal(isCaptureReady(ready, "stable"), true);
  assert.equal(isCaptureReady({ ...ready, imagesReady: false }, "stable"), false);
  assert.equal(isCaptureReady({ ...ready, loadingCount: 1 }, "stable"), false);
  assert.equal(isCaptureReady({ ...ready, geometry: "changed" }, "stable"), false);
});

test("legacy public screenshot files are absent", async () => {
  for (const file of ["screenshot-library.png", "screenshot-inspector.png", "screenshot-displays.png", "screenshot-widget.png", "screenshot-library.webp", "screenshot-inspector.webp", "screenshot-displays.webp", "screenshot-widget.webp"]) {
    await assert.rejects(stat(join(images, file)), { code: "ENOENT" });
  }
});
