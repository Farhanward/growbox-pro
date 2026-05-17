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

const caption = [
  "E2E DRAFT ONLY - do not publish.",
  "A soft baby reception setup with warm pink and gold details, ready for a polished first impression.",
  "#eventstyling #babysetup #photography #gulfcontent #تصوير_احترافي #تنسيق_حفلات",
].join("\n");

const accounts = [
  {
    platform: "instagram",
    username: "ione.twow",
    profileSlug: "profile-71b6ccdf6edcca3a",
    port: 9222,
    publishUrl: "https://www.instagram.com/create/select/",
    media: join(desktop, "WhatsApp Image 2026-05-13 at 3.18.06 PM.jpeg"),
  },
  {
    platform: "tiktok",
    username: "laflower.online@gmail.com",
    profileSlug: "profile-462492fdfa7cad04",
    port: 9223,
    publishUrl: "https://www.tiktok.com/upload",
    media: join(desktop, "growbox-e2e-small-video.mp4"),
  },
];

const reportPath = join(desktop, `GrowBox-Pro-E2E-Publish-Prepare-${Date.now()}.md`);
const results = [];

function record(name, status, details = "") {
  results.push({ name, status, details });
}

async function waitForCdp(port, timeoutMs = 25_000) {
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

async function launch(account) {
  const profileDir = join(appData, "GrowBoxPro", "isolated-browser-profiles", account.profileSlug);
  if (!existsSync(profileDir)) throw new Error(`Missing profile directory: ${profileDir}`);
  if (!existsSync(account.media)) throw new Error(`Missing media file: ${account.media}`);
  const child = spawn(chromePath, [
    `--remote-debugging-port=${account.port}`,
    `--user-data-dir=${profileDir}`,
    "--no-first-run",
    "--new-window",
    account.publishUrl,
  ], { stdio: "ignore", windowsHide: true });
  await waitForCdp(account.port);
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${account.port}`);
  const context = browser.contexts()[0];
  const page = context.pages()[0] || await context.newPage();
  await page.goto(account.publishUrl, { waitUntil: "domcontentloaded", timeout: 60_000 });
  return { child, browser, page };
}

async function close(child, browser) {
  try { await browser?.close(); } catch {}
  if (child?.pid) spawnSync("taskkill.exe", ["/pid", String(child.pid), "/T", "/F"], { stdio: "ignore" });
}

async function clickButtonLike(page, patterns, maxClicks = 1) {
  let clicked = 0;
  for (let i = 0; i < 8 && clicked < maxClicks; i++) {
    const ok = await page.evaluate((sources) => {
      const patterns = sources.map((source) => new RegExp(source, "i"));
      const candidates = [...document.querySelectorAll("button, div[role='button'], span")]
        .filter((el) => {
          const text = (el.innerText || el.textContent || "").trim();
          const rect = el.getBoundingClientRect();
          return text && rect.width > 0 && rect.height > 0 && patterns.some((re) => re.test(text));
        });
      const target = candidates[0];
      if (!target) return false;
      target.click();
      return true;
    }, patterns.map((p) => p.source));
    if (!ok) return clicked;
    clicked++;
    await page.waitForTimeout(2500);
  }
  return clicked;
}

async function setCaption(page, text) {
  return await page.evaluate((value) => {
    const selectors = [
      "textarea",
      "[contenteditable='true']",
      "div[role='textbox']",
      "input[placeholder*='caption' i]",
      "input[placeholder*='describe' i]",
    ];
    for (const selector of selectors) {
      const fields = [...document.querySelectorAll(selector)].filter((el) => {
        const rect = el.getBoundingClientRect();
        const type = el.getAttribute("type");
        return rect.width > 0 && rect.height > 0 && type !== "password";
      });
      for (const field of fields) {
        field.focus();
        if ("value" in field) {
          field.value = value;
          field.dispatchEvent(new Event("input", { bubbles: true }));
          field.dispatchEvent(new Event("change", { bubbles: true }));
          return true;
        }
        field.textContent = value;
        field.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: value }));
        return true;
      }
    }
    return false;
  }, text);
}

async function detectPublishButton(page) {
  return await page.evaluate(() => {
    const re = /^(post|publish|share|نشر|مشاركة)$/i;
    return [...document.querySelectorAll("button, div[role='button']")]
      .map((el) => (el.innerText || el.textContent || "").trim())
      .filter((text) => re.test(text));
  });
}

async function prepareInstagram(account) {
  let child, browser, page;
  try {
    ({ child, browser, page } = await launch(account));
    await page.waitForTimeout(5000);
    const fileInput = page.locator("input[type='file']").first();
    await fileInput.setInputFiles(account.media);
    record("instagram: media selected", "PASS", account.media);
    await page.waitForTimeout(6000);
    const nextClicks = await clickButtonLike(page, [/next|التالي/], 2);
    record("instagram: advanced to caption step", nextClicks > 0 ? "PASS" : "WARN", `Next clicks=${nextClicks}`);
    const captionSet = await setCaption(page, caption);
    record("instagram: caption inserted before publish", captionSet ? "PASS" : "WARN", captionSet ? "Caption field filled." : "Could not find visible caption field.");
    const publishButtons = await detectPublishButton(page);
    record("instagram: final publish was not clicked", "PASS", `Detected final buttons: ${publishButtons.join(", ") || "none"}`);
    const screenshot = join(desktop, "GrowBox-Pro-E2E-instagram-publish-prepare.png");
    await page.screenshot({ path: screenshot, fullPage: true });
    record("instagram: screenshot", "PASS", screenshot);
  } catch (error) {
    record("instagram: publish prepare", "FAIL", String(error?.stack || error));
  } finally {
    await close(child, browser);
  }
}

async function prepareTikTok(account) {
  let child, browser, page;
  try {
    ({ child, browser, page } = await launch(account));
    await page.waitForTimeout(7000);
    const fileInput = page.locator("input[type='file']").first();
    await fileInput.setInputFiles(account.media);
    record("tiktok: media selected", "PASS", account.media);
    await page.waitForTimeout(25_000);
    const captionSet = await setCaption(page, caption);
    record("tiktok: caption inserted before publish", captionSet ? "PASS" : "WARN", captionSet ? "Caption field filled." : "Could not find visible caption field.");
    const publishButtons = await detectPublishButton(page);
    record("tiktok: final publish was not clicked", "PASS", `Detected final buttons: ${publishButtons.join(", ") || "none"}`);
    const screenshot = join(desktop, "GrowBox-Pro-E2E-tiktok-publish-prepare.png");
    await page.screenshot({ path: screenshot, fullPage: true });
    record("tiktok: screenshot", "PASS", screenshot);
  } catch (error) {
    record("tiktok: publish prepare", "FAIL", String(error?.stack || error));
  } finally {
    await close(child, browser);
  }
}

async function main() {
  if (!chromePath) throw new Error("Chrome/Edge not found");
  await prepareInstagram(accounts[0]);
  await prepareTikTok(accounts[1]);
  const pass = results.filter((r) => r.status === "PASS").length;
  const warn = results.filter((r) => r.status === "WARN").length;
  const fail = results.filter((r) => r.status === "FAIL").length;
  const body = [
    "# GrowBox Pro E2E - Publish Page Preparation",
    "",
    `Date: ${new Date().toISOString()}`,
    `PASS: ${pass}`,
    `WARN: ${warn}`,
    `FAIL: ${fail}`,
    "",
    "Safety: final Publish/Post/Share button was intentionally not clicked.",
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
