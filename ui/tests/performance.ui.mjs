// UI checks for the Performance section, run against the browser preview
// (mock data from src/lib/ffmock*.ts, no backend, no hardware).
//
//   npm run build --prefix ui
//   npm run test:ui --prefix ui
//
// Needs a Chrome or Chromium on this machine (set CHROME_PATH if it is not
// found). These checks cover rendering and interaction only. They say
// nothing about real hardware, drivers or the Rust backend.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { setTimeout as sleep } from "node:timers/promises";
import puppeteer from "puppeteer-core";

const PORT = 4179;
const BASE = `http://localhost:${PORT}/`;
const chrome = [process.env.CHROME_PATH, "/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser", "C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"].find((p) => p && existsSync(p));
if (!chrome) {
  console.error("No Chrome/Chromium/Edge found. Set CHROME_PATH.");
  process.exit(2);
}

const server = spawn(process.execPath, ["node_modules/vite/bin/vite.js", "preview", "--port", String(PORT), "--strictPort"], { stdio: "ignore" });
const browser = await puppeteer.launch({ executablePath: chrome, headless: true, args: ["--no-sandbox", "--disable-gpu"] });
let failed = 0;
const results = [];

async function open(query) {
  const page = await browser.newPage();
  await page.setViewport({ width: 1500, height: 1000 });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  for (let i = 0; ; i++) {
    try {
      await page.goto(`${BASE}?splash=0&page=performance&${query}`, { waitUntil: "networkidle0" });
      break;
    } catch (e) {
      if (i > 40) throw e;
      await sleep(250);
    }
  }
  page.errors = errors;
  return page;
}

const text = (page) => page.evaluate(() => document.body.innerText);
async function waitText(page, needle, ms = 5000) {
  // Case-insensitive: CSS upper-cases some labels and innerText reflects that.
  await page.waitForFunction((n) => document.body.innerText.toLowerCase().includes(n.toLowerCase()), { timeout: ms }, needle).catch(() => {
    throw new Error(`expected to see "${needle}"`);
  });
}
/** Click the first visible button (or role=tab/radio/option) whose text starts with `label`, optionally inside an open dialog. */
async function click(page, label, { dialog = false } = {}) {
  const ok = await page.evaluate(
    (label, dialog) => {
      const root = dialog ? [...document.querySelectorAll("dialog[open]")].pop() : document;
      const el = [...(root?.querySelectorAll("button") ?? [])].find((b) => b.innerText.trim().startsWith(label) && !b.disabled && b.offsetParent !== null);
      el?.click();
      return !!el;
    },
    label,
    dialog,
  );
  if (!ok) throw new Error(`no clickable button "${label}"`);
  await sleep(120);
}
const dialogTitle = (page) => page.evaluate(() => [...document.querySelectorAll("dialog[open] h3")].pop()?.innerText ?? null);

async function check(name, fn) {
  let page;
  try {
    page = await fn();
    if (page?.errors?.length) throw new Error(`console errors: ${page.errors.slice(0, 3).join(" | ")}`);
    results.push(`ok    ${name}`);
  } catch (e) {
    failed += 1;
    results.push(`FAIL  ${name}: ${e.message}`);
  } finally {
    await page?.close().catch(() => {});
  }
}

await check("Performance navigation shows every section", async () => {
  const page = await open("");
  await waitText(page, "Performance");
  const tabs = await page.evaluate(() => [...document.querySelectorAll('[role="tab"]')].map((t) => t.innerText.trim()));
  for (const t of ["Dashboard", "Optimizer", "Live Monitor", "Game Profiles", "GPU Control", "NVIDIA Profiles", "Benchmark History"]) {
    if (!tabs.includes(t)) throw new Error(`tab "${t}" is missing (have: ${tabs.join(", ")})`);
    await click(page, t);
    const selected = await page.evaluate(() => document.querySelector('[role="tab"][aria-selected="true"]')?.innerText.trim());
    if (selected !== t) throw new Error(`clicking "${t}" selected "${selected}"`);
  }
  // The rest of the app is still reachable from the rail.
  for (const n of ["Library", "Minecraft", "Roblox", "Performance"]) await click(page, n);
  return page;
});

await check("Dashboard renders hardware from the backend data", async () => {
  const page = await open("perftab=dashboard");
  for (const t of ["Processor", "Graphics", "Memory", "Displays"]) await waitText(page, t);
  return page;
});

await check("All four themes restyle the Performance pages", async () => {
  const seen = new Set();
  let page;
  for (const look of ["obsidian", "gruvbox", "frost", "dusk"]) {
    await page?.close();
    page = await open(`perftab=optimizer&look=${look}`);
    await waitText(page, "Analyse again");
    const style = await page.evaluate(() => {
      const card = document.querySelector(".ff-card");
      const cs = getComputedStyle(card);
      return { preset: document.documentElement.dataset.preset, key: [cs.backgroundColor, cs.color, cs.borderRadius, getComputedStyle(document.body).fontFamily].join("|") };
    });
    if (style.preset !== look) throw new Error(`theme ${look} was not applied`);
    seen.add(style.key);
    if (page.errors.length) throw new Error(`console errors in ${look}: ${page.errors[0]}`);
  }
  if (seen.size !== 4) throw new Error(`only ${seen.size} distinct card styles across 4 themes: FrameForge is not following the theme variables`);
  return page;
});

await check("Hardware-unavailable states are explained, not faked", async () => {
  // A GPU whose vendor API is missing: the reason is shown, readings say why they are absent.
  let page = await open("perftab=dashboard&ffgpu=amd");
  await waitText(page, "ADLX unavailable");
  await waitText(page, "compiled without AMD ADLX support");
  for (const mode of ["amd", "intel", "none"]) {
    await page.close();
    page = await open(`perftab=dashboard&ffgpu=${mode}`);
    await waitText(page, "Processor");
    const body = await text(page);
    if (/\bNaN\b|undefined|\bnull\b/.test(body)) throw new Error(`${mode}: a missing reading rendered as NaN/undefined/null`);
    if (page.errors.length) throw new Error(`console errors (${mode}): ${page.errors[0]}`);
  }
  return page;
});

await check("Optimizer: select, confirm, apply, history, revert", async () => {
  const page = await open("perftab=optimizer&optview=actions");
  await waitText(page, "Use the High performance power plan");
  await page.click('input[type="checkbox"][aria-label^="Select"]');
  await click(page, "Review 1 change");
  if ((await dialogTitle(page)) !== "Apply 1 change?") throw new Error("the confirmation dialog did not open");
  await click(page, "Apply", { dialog: true });
  await waitText(page, "change(s) applied and recorded");
  await waitText(page, "Applied by Arcadia");
  await click(page, "History");
  await waitText(page, "Original value");
  await click(page, "Revert all");
  await waitText(page, "Reverted");
  return page;
});

await check("Optimizer: Expert level asks for explicit acknowledgement", async () => {
  const page = await open("perftab=optimizer&optview=actions");
  await waitText(page, "Level");
  await click(page, "Expert");
  if ((await dialogTitle(page)) !== "Turn on Expert level?") throw new Error("Expert was enabled without the acknowledgement dialog");
  await click(page, "Cancel", { dialog: true });
  return page;
});

await check("Background apps: closing needs confirmation, protected processes have no controls", async () => {
  const page = await open("perftab=optimizer&optview=processes");
  await waitText(page, "Why it may matter");
  await click(page, "All");
  const protectedHasControls = await page.evaluate(() => [...document.querySelectorAll("tr.prot")].some((r) => r.querySelector("button, select, input")));
  if (protectedHasControls) throw new Error("a protected process row has controls");
  await click(page, "End");
  if (!(await dialogTitle(page))?.startsWith("End ")) throw new Error("End did not ask for confirmation");
  await click(page, "Cancel", { dialog: true });
  return page;
});

await check("Live Monitor: start and stop", async () => {
  const page = await open("perftab=monitor");
  await waitText(page, "Start monitoring");
  await click(page, "Start monitoring");
  await waitText(page, "Stop");
  await click(page, "Stop");
  await waitText(page, "Start monitoring");
  return page;
});

await check("Live Monitor: PresentMon missing shows setup help, no numbers", async () => {
  const page = await open("perftab=monitor&ffpm=0");
  await waitText(page, "PresentMon");
  await waitText(page, "Open the PresentMon download page");
  if ((await text(page)).includes("Start monitoring")) throw new Error("monitoring is offered without PresentMon");
  return page;
});

await check("NVIDIA Profiles: edit, review differences, apply, restore from backup", async () => {
  const page = await open("perftab=nvidia");
  await waitText(page, "Base Profile");
  await waitText(page, "Vertical Sync");
  // Raw ids stay visible next to friendly names.
  await waitText(page, "0x00A879CF");
  await page.select('select[aria-label="Value of Vertical Sync"]', String(0x47814940));
  await waitText(page, "change not applied yet");
  await click(page, "Review and apply");
  if (!(await dialogTitle(page))?.startsWith("Apply to Base Profile")) throw new Error("the review dialog did not open");
  await waitText(page, "Force on");
  await click(page, "Apply to driver", { dialog: true });
  await waitText(page, "saved to the driver and verified");
  await click(page, "Backups");
  await waitText(page, "Before editing Base Profile");
  await click(page, "Restore", { dialog: true });
  await click(page, "Restore", { dialog: true });
  await waitText(page, "Restored and verified");
  return page;
});

await check("NVIDIA Profiles: invalid values are flagged and cannot be applied", async () => {
  const page = await open("perftab=nvidia");
  await waitText(page, "Vertical Sync");
  await page.select('select[aria-label="Value of Vertical Sync"]', "custom");
  await page.waitForSelector('input[aria-label="Value of Vertical Sync"]');
  const type = (value) =>
    page.evaluate((value) => {
      const el = document.querySelector('input[aria-label="Value of Vertical Sync"]');
      el.value = value;
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }, value);
  await type("not a number");
  await waitText(page, "Enter a number");
  await type("0x1FFFFFFFF");
  await waitText(page, "must fit in 32 bits");
  const visibleApply = await page.evaluate(() => [...document.querySelectorAll("button")].some((b) => b.innerText.startsWith("Review and apply") && !b.disabled));
  if (visibleApply) throw new Error("an invalid value can be applied");
  return page;
});

await check("NVIDIA Profiles: search by ID, filters, undocumented marker, bit editor", async () => {
  const page = await open("perftab=nvidia");
  await waitText(page, "Undocumented");
  await page.type('input[aria-label="Search settings"]', "0x1057EB71");
  await waitText(page, "Power Management - Mode");
  if ((await text(page)).includes("Vertical Sync")) throw new Error("searching by setting ID did not filter");
  await click(page, "Bits");
  if (!(await dialogTitle(page))?.endsWith("as bits")) throw new Error("the bit editor did not open");
  await click(page, "Cancel", { dialog: true });
  return page;
});

await check("NVIDIA Profiles: import dialog shows file vs driver and asks before importing", async () => {
  const page = await open("perftab=nvidia");
  await waitText(page, "Import .nip");
  await click(page, "Import .nip");
  await waitText(page, "In the driver now");
  await waitText(page, "0xDEADBEEF");
  await click(page, "Import 1 profile", { dialog: true });
  if ((await dialogTitle(page)) !== "Import into the NVIDIA driver?") throw new Error("import started without confirmation");
  await click(page, "Import", { dialog: true });
  await waitText(page, "read back from the driver");
  return page;
});

await check("NVIDIA Profiles without an NVIDIA GPU: explained, file tools still offered", async () => {
  const page = await open("perftab=nvidia&nv=none");
  await waitText(page, "NVIDIA driver profiles are not available on this PC");
  await waitText(page, "nvapi64.dll was not found");
  await waitText(page, "Open a .nip file");
  if ((await text(page)).includes("Import .nip")) throw new Error("driver import is offered without a driver");
  await click(page, "Compare two files");
  if ((await dialogTitle(page)) !== "Compare two profile files") throw new Error("compare did not open");
  return page;
});

await check("GPU Control: each vendor shows only what its API supports", async () => {
  const page = await open("perftab=gpu");
  await waitText(page, "GPU Control");
  for (const vendor of ["amd", "intel"]) {
    const p = await open(`perftab=gpu&ffgpu=${vendor}`);
    await waitText(p, vendor === "amd" ? "AMD" : "Intel");
    const body = await text(p);
    if (body.includes(".nip")) throw new Error(`${vendor}: NVIDIA .nip files are offered for a non-NVIDIA GPU`);
    if (p.errors.length) throw new Error(`console errors (${vendor}): ${p.errors[0]}`);
    await p.close();
  }
  return page;
});

await check("GPU Control: locked until the level allows it, then stage, review, apply", async () => {
  const page = await open("perftab=gpu&ffgpu=intel");
  await waitText(page, "Low latency mode");
  const select = () => page.evaluate(() => document.querySelector('select[aria-label="Low latency mode"]')?.disabled);
  if ((await select()) !== true) throw new Error("an Advanced control is editable at the Safe level");
  if (await page.evaluate(() => !!document.querySelector('[aria-label="Variable refresh rate in windowed games"]'))) throw new Error("an unsupported control has an input");
  await click(page, "Optimizer");
  await click(page, "Advanced");
  await click(page, "GPU Control");
  await waitText(page, "Low latency mode");
  await page.waitForFunction(() => document.querySelector('select[aria-label="Low latency mode"]')?.disabled === false, { timeout: 3000 }).catch(() => {
    throw new Error("the control stayed locked at the Advanced level");
  });
  if ((await page.evaluate(() => document.querySelector('input[aria-label="Sustained power limit"]')?.disabled)) !== true) throw new Error("an Expert limit is editable at the Advanced level");
  await page.evaluate(() => {
    const el = document.querySelector('select[aria-label="Low latency mode"]');
    el.value = "1";
    el.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await waitText(page, "1 change(s) not applied yet");
  await click(page, "Review and apply");
  if ((await dialogTitle(page)) !== "Apply driver controls") throw new Error("no review step before applying");
  await click(page, "Apply", { dialog: true });
  await waitText(page, "driver control(s) changed");
  await waitText(page, "Now: On");
  // Presets: the check lists what would change before anything is applied.
  await click(page, "Check and apply");
  await waitText(page, "Will change");
  await waitText(page, "Skipped");
  await click(page, "Apply preset", { dialog: true });
  await waitText(page, "read back");
  return page;
});

await check("Game Profiles: create, edit, duplicate", async () => {
  const page = await open("perftab=profiles");
  await waitText(page, "Game Profiles");
  return page;
});

await check("Benchmark History: sessions listed and comparable", async () => {
  const page = await open("perftab=benchmarks");
  await waitText(page, "Benchmark History");
  return page;
});

await browser.close();
server.kill();
console.log(results.join("\n"));
console.log(`\n${results.length - failed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
