import { createServer } from "node:http";
import { createReadStream, existsSync, statSync } from "node:fs";
import { readFile, mkdir, readdir, rm } from "node:fs/promises";
import { extname, join, relative, resolve } from "node:path";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { captureReadinessSnapshot } from "./qa-screenshots.helpers.mjs";

const root = resolve("dist");
const output = resolve("docs/images");
const port = Number(process.env.PW_SCREENSHOT_PORT || 4179);
const themes = ["dark", "light"];
const views = ["library", "inspector", "displays"];
const expectedFiles = themes.flatMap((theme) => views.map((view) => `screenshot-${view}-${theme}.webp`));

const tags = [
  { id: 1, name: "Landscape", color: "#4cc9f0" },
  { id: 2, name: "Architecture", color: "#b892ff" },
  { id: 3, name: "Night", color: "#7bd88f" },
  { id: 4, name: "Minimal", color: "#f7c948" },
];
const palettes = [
  ["#172554", "#38bdf8", "#fef3c7"], ["#3b0764", "#c084fc", "#fce7f3"],
  ["#052e16", "#22c55e", "#d9f99d"], ["#431407", "#fb923c", "#fef3c7"],
  ["#111827", "#60a5fa", "#e0e7ff"], ["#312e81", "#818cf8", "#f5f3ff"],
  ["#164e63", "#22d3ee", "#cffafe"], ["#3f1d38", "#f472b6", "#fce7f3"],
  ["#365314", "#a3e635", "#ecfccb"], ["#1c1917", "#d6d3d1", "#fafaf9"],
  ["#172554", "#2563eb", "#bfdbfe"], ["#422006", "#eab308", "#fef9c3"],
];
function imageData(index, large = false) {
  const [a, b, c] = palettes[index % palettes.length];
  const width = large ? 1600 : 640;
  const height = large ? 1000 : 400;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="${a}"/><stop offset=".55" stop-color="${b}"/><stop offset="1" stop-color="${c}"/></linearGradient><radialGradient id="r"><stop stop-color="#fff" stop-opacity=".45"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient></defs><rect width="100%" height="100%" fill="url(#g)"/><circle cx="${width * .72}" cy="${height * .23}" r="${height * .18}" fill="url(#r)"/><path d="M0 ${height * .78} Q${width * .2} ${height * .6} ${width * .4} ${height * .79} T${width} ${height * .7} V${height}H0Z" fill="#07111f" fill-opacity=".42"/><path d="M0 ${height * .88} Q${width * .25} ${height * .68} ${width * .5} ${height * .89} T${width} ${height * .8} V${height}H0Z" fill="#020617" fill-opacity=".58"/><path d="M${width * .12} ${height * .78} L${width * .12} ${height * .48} L${width * .24} ${height * .38} L${width * .36} ${height * .48} L${width * .36} ${height * .78}" fill="#020617" fill-opacity=".35"/></svg>`;
  return `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;
}
const wallpapers = Array.from({ length: 12 }, (_, index) => ({
  id: index + 1,
  path: `C:\\demo\\purewall-${String(index + 1).padStart(2, "0")}.jpg`,
  hash: `demo-hash-${index}`,
  source: "Demo Collection",
  display_title: ["Azure Morning", "Violet Geometry", "Forest After Rain", "Amber Horizon", "Quiet Blue", "Indigo Lines", "Coastal Air", "Rose Dusk", "Lime Ridge", "Stone Light", "Open Sky", "Golden Hour"][index],
  rating: [1, 0, -1, 0, 1, 0, 0, 1, 0, -1, 0, 1][index],
  play_count: 4 + index * 3,
  last_played: index < 6 ? "2026-06-18T12:00:00Z" : null,
  created_at: `2026-06-${String(1 + index).padStart(2, "0")}T10:00:00Z`,
  blacklisted: false,
  width: 3840,
  height: 2160,
  file_size: 4_800_000 + index * 420_000,
  tags: [tags[index % tags.length], ...(index % 3 === 0 ? [tags[(index + 1) % tags.length]] : [])],
}));
const images = new Map(wallpapers.map((wallpaper, index) => [wallpaper.path, imageData(index)]));
const previews = new Map(wallpapers.map((wallpaper, index) => [wallpaper.path, imageData(index, true)]));

function mockInvoke(command, args = {}) {
  const wallpapers = window.__PW_WALLPAPERS;
  const tags = window.__PW_TAGS;
  const images = window.__PW_IMAGES;
  const previews = window.__PW_PREVIEWS;
  if (command === "plugin:event|listen" || command === "plugin:event|unlisten") return command.endsWith("listen") ? 1 : null;
  if (command === "bootstrap_active_wallpaper") return { path: wallpapers[0].path, wallpaper: wallpapers[0], thumbnail_path: images[wallpapers[0].path], preview_path: previews[wallpapers[0].path], preview_state: "ready" };
  if (command === "get_wallpapers_page") {
    let items = wallpapers;
    if (args.filter === "liked") items = wallpapers.filter((w) => w.rating === 1);
    if (args.filter === "disliked") items = wallpapers.filter((w) => w.rating === -1);
    if (args.filter?.startsWith("tag:")) items = wallpapers.filter((w) => w.tags.some((tag) => `tag:${tag.id}` === args.filter));
    if (args.search) items = wallpapers.filter((w) => `${w.display_title} ${w.path}`.toLowerCase().includes(String(args.search).toLowerCase()));
    const offset = Number(args.offset || 0);
    const limit = Number(args.limit || 96);
    return { items: items.slice(offset, offset + limit), total: items.length, offset, limit, has_more: false };
  }
  if (command === "get_tags") return tags;
  if (command === "get_collections") return [{ id: 1, name: "Featured", color: "#5b8def", wallpaper_count: 6 }, { id: 2, name: "Weekend", color: "#ff7a59", wallpaper_count: 4 }];
  if (command === "get_stats") return { total: wallpapers.length, liked: 4, disliked: 2, blacklisted: 0, total_plays: 138 };
  if (command === "get_displays") return [{ index: 0, id: "DISPLAY-1", left: 0, top: 0, width: 2560, height: 1440, current_wallpaper: wallpapers[0].path }, { index: 1, id: "DISPLAY-2", left: 2560, top: 0, width: 1920, height: 1080, current_wallpaper: wallpapers[1].path }];
  if (command === "get_display_mode") return "all";
  if (command === "set_display_mode") return args.mode || "all";
  if (command === "get_focus_mode_status") return { enabled: false, fullscreen_detected: false, auto_paused: false };
  if (command === "set_focus_mode_enabled") return { enabled: Boolean(args.enabled), fullscreen_detected: false, auto_paused: false };
  if (command === "is_paused") return false;
  if (command === "get_rotation_interval") return 900;
  if (command === "get_yearly_stats") return { year: 2026, total_plays: 138, unique_wallpapers: 12, liked_plays: 54, monthly: Array.from({ length: 12 }, (_, i) => ({ month: i + 1, plays: 5 + i * 2 })), top_wallpapers: wallpapers.slice(0, 4).map((w, i) => ({ path: w.path, plays: 20 - i, rating: w.rating })) };
  if (command === "load_thumbnails_batch") return Object.fromEntries((args.paths || []).map((path) => [path, images[path] || Object.values(images)[0]]));
  if (command === "load_preview_image") return previews[args.path] || Object.values(previews)[0];
  if (command === "get_image_metadata") return { width: 3840, height: 2160, file_size: 5_200_000 };
  if (command === "get_shell_metadata") return { authors: "PureWall Studio", copyright: "Demo Collection · 2026", comment: "Synthetic preview data" };
  if (command === "list_library_sources") return [{ path: "C:\\demo", source: "imported-folder", status: "online", available_count: 12, unavailable_count: 0, last_scan_at: "2026-06-18T12:00:00Z", last_error: null }];
  if (command === "is_autostart_enabled") return false;
  if (command === "read_cli_log") return "No command-line actions recorded in demo mode.";
  if (command === "get_library_source_status") return null;
  if (command === "get_current_wallpaper") return wallpapers[0].path;
  if (command === "get_wallpaper_by_path") return wallpapers.find((w) => w.path === args.path) || null;
  if (["prewarm_preview_images", "toggle_widget", "hide_widget", "set_current_wallpaper", "run_playback_action", "set_rotation_interval", "set_autostart", "check_for_update", "install_update"].includes(command)) return command === "check_for_update" ? null : (command === "run_playback_action" ? { action: args.action, current_wallpaper_path: wallpapers[0].path, rating: wallpapers[0].rating, paused: false } : null);
  window.__PW_UNKNOWN_COMMANDS.push(command);
  throw new Error(`Unknown mocked Tauri command: ${command}`);
}

async function loadPlaywright() {
  const candidates = [];
  if (process.env.PW_PLAYWRIGHT_PATH) candidates.push(process.env.PW_PLAYWRIGHT_PATH);
  try {
    const require = createRequire(import.meta.url);
    candidates.push(require.resolve("playwright"));
  } catch {
    // Playwright is an optional maintainer-side QA tool.
  }
  candidates.push(join(process.cwd(), "node_modules/playwright/index.mjs"));
  for (const cacheRoot of [
    process.env.LOCALAPPDATA && join(process.env.LOCALAPPDATA, "npm-cache/_npx"),
    process.env.USERPROFILE && join(process.env.USERPROFILE, "AppData/Local/npm-cache/_npx"),
    process.env.HOME && join(process.env.HOME, ".npm/_npx"),
  ].filter(Boolean)) {
    try {
      for (const entry of await readdir(cacheRoot)) candidates.push(join(cacheRoot, entry, "node_modules/playwright/index.mjs"));
    } catch {
      // An absent npm cache is normal; continue to the explicit override/default package.
    }
  }
  for (const candidate of candidates) {
    try { return await import(pathToFileURL(resolve(candidate)).href); } catch { /* try next */ }
  }
  throw new Error("Playwright was not found. Install it in the workspace or set PW_PLAYWRIGHT_PATH to its index.mjs (Windows example: C:\\tools\\playwright\\index.mjs).");
}

function chromeCandidates() {
  const env = process.env;
  return [
    env.PW_CHROME_PATH,
    env.CHROME_PATH,
    env.LOCALAPPDATA && join(env.LOCALAPPDATA, "Google/Chrome/Application/chrome.exe"),
    env.PROGRAMFILES && join(env.PROGRAMFILES, "Google/Chrome/Application/chrome.exe"),
    env["PROGRAMFILES(X86)"] && join(env["PROGRAMFILES(X86)"], "Google/Chrome/Application/chrome.exe"),
    "/usr/bin/google-chrome", "/usr/bin/chromium", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  ].filter(Boolean);
}
function findChrome() {
  const direct = chromeCandidates().find((candidate) => existsSync(candidate));
  if (direct) return direct;
  const command = process.platform === "win32" ? "where" : "which";
  for (const binary of process.platform === "win32" ? ["chrome.exe", "msedge.exe"] : ["google-chrome", "chromium", "chromium-browser"]) {
    const result = spawnSync(command, [binary], { encoding: "utf8", windowsHide: true });
    if (result.status === 0 && result.stdout.trim()) return result.stdout.trim().split(/\r?\n/)[0];
  }
  return null;
}

function serveStatic(server, request, response) {
  const pathname = decodeURIComponent((request.url || "/").split("?")[0]);
  const requested = pathname === "/" || !extname(pathname) ? "index.html" : pathname.replace(/^[/\\]+/, "");
  const file = resolve(root, requested);
  const pathRelativeToRoot = relative(root, file);
  if (pathRelativeToRoot.startsWith("..") || !existsSync(file) || !statSync(file).isFile()) {
    response.writeHead(404); response.end("Not found"); return;
  }
  const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2" };
  response.writeHead(200, { "Content-Type": types[extname(file)] || "application/octet-stream" });
  createReadStream(file).pipe(response);
}

async function injectCaptureVisualPatch(page) {
  await page.addStyleTag({ content: `
    .living-gallery-shell .workbench-grid,
    .living-gallery-shell.inspector-is-open .workbench-grid { grid-template-columns: 224px minmax(0, 1fr) 320px !important; }
    .living-gallery-shell:not(.inspector-is-open) .workbench-grid { grid-template-columns: 224px minmax(0, 1fr) !important; }
    .living-gallery-shell .sidebar-item__label { overflow: visible !important; text-overflow: clip !important; white-space: nowrap !important; }
    .statusbar-shell > [data-qa-capture-only="synthetic-demo-data"] { position: static; flex: 0 0 auto; margin: 0 12px; }
  ` });
}

function addCaptureBadge(page, theme) {
  return page.evaluate((themeName) => {
    const badge = document.createElement("div");
    badge.textContent = `Synthetic demo data · ${themeName}`;
    badge.setAttribute("data-qa-capture-only", "synthetic-demo-data");
    Object.assign(badge.style, { zIndex: "2147483647", padding: "6px 10px", borderRadius: "999px", font: "600 11px/1.2 Arial,sans-serif", letterSpacing: ".02em", color: "#fff", background: "rgba(15,23,42,.78)", boxShadow: "0 2px 10px rgba(0,0,0,.22)", pointerEvents: "none" });
    const host = document.querySelector(".statusbar-shell");
    if (host) {
      const middle = host.children[Math.floor(host.children.length / 2)] || null;
      host.insertBefore(badge, middle);
    } else {
      Object.assign(badge.style, { position: "fixed", left: "14px", bottom: "44px" });
      document.body.appendChild(badge);
    }
  }, theme);
}

async function waitForCaptureReady(page, label) {
  await page.evaluate(async () => { if (document.fonts?.ready) await document.fonts.ready; });
  await page.waitForFunction(() => {
    const visible = (element) => { const rect = element.getBoundingClientRect(); const style = getComputedStyle(element); return rect.width > 0 && rect.height > 0 && style.visibility !== "hidden" && style.display !== "none"; };
    const images = Array.from(document.images).filter(visible);
    const loading = [".workspace-state--loading", ".loading-state", ".gallery-page-loading", ".tile-loading", ".living-stage__placeholder", '[aria-busy="true"]'].flatMap((selector) => Array.from(document.querySelectorAll(selector)).filter(visible));
    const geometry = Array.from(document.querySelectorAll("body *")).filter(visible).map((element) => { const rect = element.getBoundingClientRect(); return `${element.tagName}:${Math.round(rect.left)},${Math.round(rect.top)},${Math.round(rect.width)},${Math.round(rect.height)}`; }).join("|");
    const current = { imagesReady: images.length > 0 && images.every((image) => image.naturalWidth > 0 && image.naturalHeight > 0), loadingCount: loading.length, geometry };
    const stable = window.__PW_LAST_CAPTURE_GEOMETRY === geometry;
    window.__PW_LAST_CAPTURE_GEOMETRY = geometry;
    return current.imagesReady && current.loadingCount === 0 && stable;
  }, undefined, { timeout: 15000, polling: 100 });
  await page.evaluate(async () => {
    const visible = (element) => { const rect = element.getBoundingClientRect(); const style = getComputedStyle(element); return rect.width > 0 && rect.height > 0 && style.visibility !== "hidden" && style.display !== "none"; };
    await Promise.all(Array.from(document.images).filter(visible).map((image) => image.decode()));
  });
  const snapshot = await page.evaluate(`(${captureReadinessSnapshot.toString()})(document)`);
  if (!snapshot.imagesReady || snapshot.loadingCount !== 0 || !snapshot.geometry) throw new Error(`Capture readiness failed for ${label}`);
  return snapshot;
}

async function clearCardHoverAndAssert(page, card) {
  const cardId = await card.getAttribute("data-wallpaper-id");
  await page.mouse.move(1590, 990);
  await page.waitForFunction((id) => {
    const card = document.querySelector(`[data-wallpaper-id="${id}"]`);
    if (!card || card.matches(":hover")) return false;
    const overlay = card.querySelector(".tile-overlay");
    const overlayHidden = !overlay || getComputedStyle(overlay).display === "none";
    const selectHidden = !Array.from(card.querySelectorAll(".select-dot")).some((control) => {
      const rect = control.getBoundingClientRect();
      const style = getComputedStyle(control);
      return rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden";
    });
    return overlayHidden && selectHidden;
  }, cardId, { timeout: 5000, polling: 100 });
  const overlap = await card.evaluate((element) => {
    const cardRect = element.getBoundingClientRect();
    return Array.from(element.querySelectorAll(".tile-overlay, .select-dot")).some((control) => {
      const rect = control.getBoundingClientRect();
      const style = getComputedStyle(control);
      const visible = rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden";
      return visible && rect.left < cardRect.right && rect.right > cardRect.left && rect.top < cardRect.bottom && rect.bottom > cardRect.top;
    });
  });
  if (overlap) throw new Error("Hover-only tile overlay or select control overlaps the selected card");
}

async function assertCaptureOnlyLayout(page) {
  const result = await page.evaluate(() => {
    const host = document.querySelector(".statusbar-shell");
    const badge = document.querySelector('[data-qa-capture-only="synthetic-demo-data"]');
    const labels = Array.from(document.querySelectorAll(".sidebar-item__label")).map((element) => ({ text: element.textContent?.trim() || "", clipped: element.scrollWidth > element.clientWidth || getComputedStyle(element).textOverflow === "ellipsis" }));
    return { badgeInStatusbar: Boolean(host && badge && badge.parentElement === host), labels };
  });
  if (!result.badgeInStatusbar) throw new Error("Synthetic badge is not a normal statusbar child");
  const clipped = result.labels.filter(({ clipped }) => clipped).map(({ text }) => text);
  if (clipped.length) throw new Error(`Capture sidebar labels are clipped: ${clipped.join(", ")}`);
}

function webpDimensions(buffer) {
  if (buffer.toString("ascii", 0, 4) !== "RIFF" || buffer.toString("ascii", 8, 12) !== "WEBP") throw new Error("not a WebP");
  let offset = 12;
  while (offset + 8 <= buffer.length) {
    const type = buffer.toString("ascii", offset, offset + 4);
    const size = buffer.readUInt32LE(offset + 4);
    const data = offset + 8;
    if (type === "VP8X" && size >= 10) return { width: 1 + buffer[data + 4] + (buffer[data + 5] << 8) + (buffer[data + 6] << 16), height: 1 + buffer[data + 7] + (buffer[data + 8] << 8) + (buffer[data + 9] << 16) };
    if (type === "VP8 " && size >= 10 && buffer[data + 3] === 0x9d && buffer[data + 4] === 0x01 && buffer[data + 5] === 0x2a) return { width: buffer.readUInt16LE(data + 6) & 0x3fff, height: buffer.readUInt16LE(data + 8) & 0x3fff };
    if (type === "VP8L" && size >= 5) return { width: 1 + (((buffer[data + 1] | (buffer[data + 2] << 8)) & 0x3fff)), height: 1 + ((((buffer[data + 2] >> 6) | (buffer[data + 3] << 2) | (buffer[data + 4] << 10)) & 0x3fff)) };
    offset = data + size + (size % 2);
  }
  throw new Error("WebP dimensions chunk missing");
}

async function captureWebp(page, path) {
  try {
    await page.screenshot({ path, type: "webp", quality: 92 });
    return;
  } catch (directError) {
    const temporaryPng = join(tmpdir(), `purewall-qa-${process.pid}-${path.split(/[\\\\/]/).pop()}.png`);
    try {
      await page.screenshot({ path: temporaryPng, type: "png" });
      const code = `from PIL import Image; Image.open(${JSON.stringify(temporaryPng)}).convert("RGB").save(${JSON.stringify(path)}, "WEBP", quality=92, method=6)`;
      const commands = process.env.PYTHON ? [[process.env.PYTHON, []]] : (process.platform === "win32" ? [["py", ["-3"]], ["python", []], ["python3", []]] : [["python3", []], ["python", []]]);
      let converted = false;
      let lastError = directError;
      for (const [command, prefix] of commands) {
        const result = spawnSync(command, [...prefix, "-c", code], { encoding: "utf8", windowsHide: true });
        if (result.status === 0) { converted = true; break; }
        lastError = new Error(result.stderr || result.error?.message || `Could not run ${command}`);
      }
      if (!converted) throw new Error(`Chromium WebP screenshot encoding failed (${lastError.message}); PIL conversion was unavailable.`);
    } finally {
      await rm(temporaryPng, { force: true });
    }
  }
}

async function validateImages(playwright, page) {
  const files = (await import("node:fs/promises")).readdir(output);
  const actual = (await files).filter((file) => file.endsWith(".webp")).sort();
  if (actual.length !== expectedFiles.length || actual.some((file, index) => file !== [...expectedFiles].sort()[index])) throw new Error(`Expected exactly six WebPs: ${actual.join(", ")}`);
  for (const file of actual) {
    const buffer = await readFile(join(output, file));
    if (buffer.length <= 20 * 1024) throw new Error(`${file} is only ${buffer.length} bytes`);
    const dimensions = webpDimensions(buffer);
    if (dimensions.width !== 1600 || dimensions.height !== 1000) throw new Error(`${file} is ${dimensions.width}x${dimensions.height}`);
    const decoded = await page.evaluate(async (bytes) => {
      const blob = new Blob([new Uint8Array(bytes)], { type: "image/webp" });
      const url = URL.createObjectURL(blob);
      try { const image = new Image(); image.src = url; await image.decode(); return { width: image.naturalWidth, height: image.naturalHeight }; } finally { URL.revokeObjectURL(url); }
    }, [...buffer]);
    if (decoded.width !== 1600 || decoded.height !== 1000) throw new Error(`${file} did not decode at 1600x1000`);
  }
  for (const view of views) {
    const dark = await readFile(join(output, `screenshot-${view}-dark.webp`));
    const light = await readFile(join(output, `screenshot-${view}-light.webp`));
    if (dark.equals(light)) throw new Error(`${view} dark/light pair is identical`);
  }
}

let server;
let browser;
try {
  if (!existsSync(join(root, "index.html"))) throw new Error("dist/index.html is missing; run npm run build first");
  const playwright = await loadPlaywright();
  server = createServer((request, response) => serveStatic(server, request, response));
  await new Promise((done) => server.listen(port, "127.0.0.1", done));
  await mkdir(output, { recursive: true });
  for (const oldFile of ["screenshot-library.png", "screenshot-inspector.png", "screenshot-displays.png", "screenshot-widget.png", "screenshot-library.webp", "screenshot-inspector.webp", "screenshot-displays.webp", "screenshot-widget.webp"]) await rm(join(output, oldFile), { force: true });
  const executablePath = findChrome();
  try {
    browser = await playwright.chromium.launch({ headless: true, ...(executablePath ? { executablePath } : {}), args: ["--force-device-scale-factor=1"] });
  } catch (error) {
    if (executablePath) throw error;
    throw new Error(`${error.message}\nNo bundled browser was available. Install Chromium for Playwright or set PW_CHROME_PATH to Chrome (Windows fallback: C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe).`);
  }
  for (const theme of themes) {
    const context = await browser.newContext({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 1, colorScheme: theme });
    const pageErrors = [];
    const blockedOutboundRequests = [];
    await context.route("**/*", async (route) => {
      const requestUrl = new URL(route.request().url());
      const loopback = ["127.0.0.1", "localhost", "::1"].includes(requestUrl.hostname);
      if ((requestUrl.protocol === "http:" || requestUrl.protocol === "https:") && !loopback) {
        blockedOutboundRequests.push(requestUrl.href);
        await route.abort("blockedbyclient");
      } else {
        await route.continue();
      }
    });
    context.on("page", (newPage) => newPage.on("pageerror", (error) => pageErrors.push(error)));
    await context.addInitScript(({ wallpapers: demoWallpapers, tags: demoTags, images: demoImages, previews: demoPreviews, themeName }) => {
      try {
        localStorage.setItem("purewall-theme", themeName);
        localStorage.setItem("purewall-workspace-mode", "workbench");
      } catch {
        // The init script also runs for about:blank before navigation; the real document is seeded.
      }
      window.__PW_WALLPAPERS = demoWallpapers;
      window.__PW_TAGS = demoTags;
      window.__PW_IMAGES = demoImages;
      window.__PW_PREVIEWS = demoPreviews;
      window.__PW_UNKNOWN_COMMANDS = [];
      const callbacks = new Map(); let callbackId = 0;
      window.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } }, invoke: async (command, args = {}) => window.__PUREWALL_DEMO_INVOKE(command, args), transformCallback: (callback) => { const id = ++callbackId; callbacks.set(id, callback); return id; }, unregisterCallback: (id) => callbacks.delete(id), convertFileSrc: (value) => String(value).startsWith("data:") ? value : value };
    }, { wallpapers, tags, images: Object.fromEntries(images), previews: Object.fromEntries(previews), themeName: theme });
    await context.addInitScript(`window.__PUREWALL_DEMO_INVOKE = ${mockInvoke.toString()};`);
    const page = await context.newPage();
    page.on("console", (message) => { if (message.type() === "error") pageErrors.push(new Error(message.text())); });
    await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: "networkidle" });
    await page.locator(".wallpaper-card").first().waitFor({ state: "visible", timeout: 15000 });
    if (await page.locator("html").getAttribute("data-theme") !== theme) throw new Error(`Expected data-theme=${theme}`);
    if (await page.locator("html").getAttribute("data-workspace-mode") !== "workbench") throw new Error("Expected workbench workspace mode");
    await injectCaptureVisualPatch(page);
    await addCaptureBadge(page, theme);
    await waitForCaptureReady(page, `${theme} library`);
    await assertCaptureOnlyLayout(page);
    await captureWebp(page, join(output, `screenshot-library-${theme}.webp`));
    const selectedCard = page.locator(".wallpaper-card").nth(5);
    await selectedCard.click();
    await page.locator(".inspector-preview").waitFor({ state: "visible", timeout: 10000 });
    await waitForCaptureReady(page, `${theme} inspector`);
    await clearCardHoverAndAssert(page, selectedCard);
    await assertCaptureOnlyLayout(page);
    await captureWebp(page, join(output, `screenshot-inspector-${theme}.webp`));
    await page.getByRole("button", { name: "Displays", exact: true }).click();
    await page.locator(".system-inspector-shell").waitFor({ state: "visible", timeout: 10000 });
    await waitForCaptureReady(page, `${theme} displays`);
    await assertCaptureOnlyLayout(page);
    await captureWebp(page, join(output, `screenshot-displays-${theme}.webp`));
    const unknown = await page.evaluate(() => window.__PW_UNKNOWN_COMMANDS);
    if (unknown.length) throw new Error(`Unknown mocked Tauri commands: ${unknown.join(", ")}`);
    if (pageErrors.length) throw new Error(`Page errors during ${theme} capture: ${pageErrors.map((error) => error.message).join("; ")}`);
    if (blockedOutboundRequests.length) console.warn(`Blocked ${blockedOutboundRequests.length} non-loopback request(s) during ${theme} capture`);
    await context.close();
  }
  const validationContext = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
  const validationPage = await validationContext.newPage();
  await validateImages(playwright, validationPage);
  await validationContext.close();
  console.log(`Wrote and validated ${expectedFiles.length} WebPs at ${output}`);
} finally {
  if (browser) await browser.close().catch(() => {});
  if (server) await new Promise((done) => server.close(() => done()));
}
