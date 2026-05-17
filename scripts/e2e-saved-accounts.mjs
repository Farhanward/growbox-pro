import { spawn, spawnSync } from "node:child_process";
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = "C:\\Projects\\reach-optimizer-pro";
const appData = process.env.APPDATA;
const desktop = join(process.env.USERPROFILE || process.env.HOME || root, "Desktop");
const chromePath = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
].find((path) => existsSync(path));

const accounts = [
  {
    platform: "instagram",
    username: "ione.twow",
    profileLabel: "ione.twow",
    profileSlug: "profile-71b6ccdf6edcca3a",
    port: 9222,
    homeUrl: "https://www.instagram.com/",
    profileUrl: "https://www.instagram.com/ione.twow/",
    publishUrl: "https://www.instagram.com/create/select/",
    samplePostUrl: "https://www.instagram.com/p/DYNtlytgsCk/",
  },
  {
    platform: "tiktok",
    username: "laflower.online@gmail.com",
    profileLabel: "laflower.online@gmail.com",
    profileSlug: "profile-462492fdfa7cad04",
    port: 9223,
    homeUrl: "https://www.tiktok.com/",
    profileUrl: "https://www.tiktok.com/",
    publishUrl: "https://www.tiktok.com/upload",
    samplePostUrl: "https://vt.tiktok.com/ZSxeNUkJu/",
  },
];

const reportPath = join(desktop, `GrowBox-Pro-E2E-Saved-Accounts-${Date.now()}.md`);
const results = [];

function record(name, status, details = "") {
  results.push({ name, status, details });
}

async function waitForCdp(port, timeoutMs = 20_000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (res.ok) return true;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`CDP ${port} did not become ready`);
}

async function launchChrome(account, url) {
  const profileDir = join(appData, "GrowBoxPro", "isolated-browser-profiles", account.profileSlug);
  if (!existsSync(profileDir)) {
    throw new Error(`Missing profile directory: ${profileDir}`);
  }
  const child = spawn(chromePath, [
    `--remote-debugging-port=${account.port}`,
    `--user-data-dir=${profileDir}`,
    "--no-first-run",
    "--new-window",
    url,
  ], {
    stdio: "ignore",
    detached: false,
    windowsHide: true,
  });
  await waitForCdp(account.port);
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${account.port}`);
  const context = browser.contexts()[0];
  const page = context.pages()[0] || await context.newPage();
  return { child, browser, page, profileDir };
}

async function closeChrome(child, browser) {
  try { await browser?.close(); } catch {}
  if (child?.pid) {
    spawnSync("taskkill.exe", ["/pid", String(child.pid), "/T", "/F"], { stdio: "ignore" });
  }
}

async function sessionProbe(account) {
  let child, browser, page;
  try {
    ({ child, browser, page } = await launchChrome(account, account.homeUrl));
    await page.goto(account.profileUrl, { waitUntil: "domcontentloaded", timeout: 60_000 });
    await page.waitForTimeout(5000);
    const url = page.url();
    const state = await page.evaluate((platform) => {
      const text = document.body.innerText || "";
      const loginInput = Boolean(document.querySelector("input[name='username'], input[name='email'], input[type='password']"));
      const loginText = /log in|sign up|تسجيل الدخول|login/i.test(text);
      const uploadText = /upload|تحميل|create|إنشاء/i.test(text);
      const profileText = text.slice(0, 1000);
      return { loginInput, loginText, uploadText, profileText, title: document.title, platform };
    }, account.platform);
    const loggedInLikely = !/\/login|accounts\/login|signup/i.test(url) && !state.loginInput;
    record(`${account.platform}: saved profile launches`, "PASS", `Profile ${account.profileSlug}, final URL ${url}`);
    record(`${account.platform}: login state`, loggedInLikely ? "PASS" : "WARN", loggedInLikely ? "No login form detected." : `Login form or login URL detected. URL=${url}`);

    await page.goto(account.publishUrl, { waitUntil: "domcontentloaded", timeout: 60_000 });
    await page.waitForTimeout(5000);
    const publishUrl = page.url();
    const publishState = await page.evaluate(() => {
      const text = document.body.innerText || "";
      return {
        loginInput: Boolean(document.querySelector("input[name='username'], input[name='email'], input[type='password']")),
        hasUploadCue: /upload|select from computer|drag|تحميل|اختيار|create|إنشاء/i.test(text),
        title: document.title,
        text: text.slice(0, 1000),
      };
    });
    const publishOk = !/\/login|accounts\/login|signup/i.test(publishUrl) && !publishState.loginInput && publishState.hasUploadCue;
    record(`${account.platform}: publish page opens in same profile`, publishOk ? "PASS" : "WARN", `URL=${publishUrl}; upload cue=${publishState.hasUploadCue}; login input=${publishState.loginInput}`);
  } catch (error) {
    record(`${account.platform}: session probe`, "FAIL", String(error?.stack || error));
  } finally {
    await closeChrome(child, browser);
  }
}

async function fetcherProbe(account) {
  const node = join(root, "src-tauri", "resources", "node", "node.exe");
  const script = join(root, "src-tauri", "resources", "playwright-fetcher.mjs");
  let child, browser, page;
  try {
    ({ child, browser, page } = await launchChrome(account, account.homeUrl));
    await page.waitForTimeout(2000);
    const proc = spawn(node, [script, account.platform, account.samplePostUrl, String(account.port)], {
      cwd: join(root, "src-tauri", "resources"),
      windowsHide: true,
    });
    let stdout = "";
    let stderr = "";
    proc.stdout.on("data", (chunk) => { stdout += chunk.toString("utf8"); });
    proc.stderr.on("data", (chunk) => { stderr += chunk.toString("utf8"); });
    const code = await new Promise((resolve) => proc.on("close", resolve));
    const hasResult = stdout.includes("\"__result\":true");
    const logPath = join(desktop, `GrowBox-Pro-E2E-${account.platform}-saved-fetcher.txt`);
    writeFileSync(logPath, stdout + "\n--- STDERR ---\n" + stderr, "utf8");
    record(`${account.platform}: data fetcher through saved session`, hasResult ? "PASS" : "WARN", hasResult ? `Result JSON captured. Log=${logPath}` : `No result JSON. Exit=${code}. Log=${logPath}. ${stderr.trim().slice(0, 400)}`);
  } catch (error) {
    record(`${account.platform}: data fetcher through saved session`, "FAIL", String(error?.stack || error));
  } finally {
    await closeChrome(child, browser);
  }
}

async function main() {
  if (!chromePath) throw new Error("Chrome/Edge not found");
  for (const account of accounts) {
    await sessionProbe(account);
    await fetcherProbe(account);
  }
  const pass = results.filter((r) => r.status === "PASS").length;
  const warn = results.filter((r) => r.status === "WARN").length;
  const fail = results.filter((r) => r.status === "FAIL").length;
  const body = [
    "# GrowBox Pro E2E - Saved Test Accounts",
    "",
    `Date: ${new Date().toISOString()}`,
    `PASS: ${pass}`,
    `WARN: ${warn}`,
    `FAIL: ${fail}`,
    "",
    "## Accounts",
    "- instagram: @ione.twow / profile: ione.twow",
    "- tiktok: @laflower.online@gmail.com / profile: laflower.online@gmail.com",
    "",
    "## Results",
    ...results.map((r) => `- ${r.status}: ${r.name}\n  ${r.details.replace(/\n/g, "\n  ")}`),
    "",
  ].join("\n");
  writeFileSync(reportPath, body, "utf8");
  console.log(body);
  console.log(`REPORT_PATH=${reportPath}`);
  if (fail) process.exit(1);
}

main().catch((error) => {
  record("runner", "FAIL", String(error?.stack || error));
  const body = results.map((r) => `- ${r.status}: ${r.name}\n  ${r.details.replace(/\n/g, "\n  ")}`).join("\n");
  writeFileSync(reportPath, body, "utf8");
  console.error(body);
  console.error(`REPORT_PATH=${reportPath}`);
  process.exit(1);
});
