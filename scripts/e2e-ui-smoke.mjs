import { spawn, spawnSync } from "node:child_process";
import { existsSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const desktop = join(process.env.USERPROFILE || process.env.HOME || root, "Desktop");
const reportPath = join(desktop, `GrowBox-Pro-E2E-UI-${Date.now()}.md`);
const mediaPath = join(desktop, "Baby لولوه دشتي.mp4");
const chromeCandidates = [
  process.env.CHROME_PATH,
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
].filter(Boolean);
const chromePath = chromeCandidates.find((path) => existsSync(path));

if (!chromePath) {
  throw new Error("Chrome/Edge executable was not found for Playwright UI E2E.");
}

const results = [];
const calls = [];

function pass(name, details = "") {
  results.push({ name, status: "PASS", details });
}

function fail(name, error) {
  results.push({ name, status: "FAIL", details: String(error?.stack || error) });
}

async function step(name, fn) {
  try {
    await fn();
    pass(name);
  } catch (error) {
    fail(name, error);
    throw error;
  }
}

function waitForServer(url, timeoutMs = 30_000) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const tick = async () => {
      try {
        const res = await fetch(url);
        if (res.ok) return resolve();
      } catch {}
      if (Date.now() - started > timeoutMs) {
        return reject(new Error(`Timed out waiting for ${url}`));
      }
      setTimeout(tick, 500);
    };
    tick();
  });
}

function tauriMock(media) {
  return `
(() => {
  const calls = [];
  let visionReady = false;
  let loginSaved = false;
  let browserRunning = false;
  let evolutionActive = false;
  let backgroundActive = false;
  let reportId = 10;
  const now = new Date().toISOString();
  const arCaption = "تجهيز استقبال مولود بلمسة وردية وذهبية مرتبة، تفاصيل ناعمة تصلح لأول ثانيتين وتخلي المشاهد يوقف عندها.";
  const enCaption = "A polished baby reception setup with soft pink florals, warm gold accents, and a clear first-frame hook for quick saves and shares.";
  const tags = ["#تصوير_احترافي", "#تصوير_مناسبات", "#تنسيق_حفلات", "#تصوير_منتجات", "#photography", "#eventstyling", "#babysetup", "#gulfcontent"];
  function output(language) {
    const en = language === "en";
    const caption = en ? enCaption : arCaption;
    return {
      summary: en ? "Evidence-based launch card for a new account baseline." : "بطاقة نشر مبنية على وصف Vision ومسار حساب جديد.",
      success_score: 82,
      suggested_time: en ? "8:30 PM" : "8:30 مساء",
      timezone: "Asia/Riyadh",
      geo_profile: { countries: ["KW", "SA"], primary_country: "KW", timezone: "Asia/Riyadh", locale: en ? "en" : "ar", ip_alignment_note: "mock" },
      hashtags: tags.map((tag, index) => ({ tag, velocity: 80 - index, relevance: 88 - index, reason: en ? "Relevant to media and niche." : "مرتبط بالمحتوى والنيتش." })),
      instagram: { platform: "Instagram", caption, hashtags: tags.slice(0, 8), suggested_time: en ? "8:30 PM" : "8:30 مساء", visual_note: en ? "Vision approved." : "وصف Vision معتمد." },
      tiktok: { platform: "TikTok", caption, hashtags: tags.slice(0, 8), suggested_time: en ? "9:00 PM" : "9:00 مساء", visual_note: en ? "Vision approved." : "وصف Vision معتمد." },
      virality_indicators: { hook_strength: "Medium", shareability_factor: "High", predicted_trend_alignment: "Rising", strategic_score: 78 },
      cards: [
        {
          platform_info: { name: "Instagram", post_id: "GBX-E2E" },
          niche_context: { account_type: en ? "Photography" : "تصوير", region: "Gulf" },
          content_payload: { caption, hashtags: { velocity: tags.slice(0, 4), relevance: tags.slice(4, 8) } },
          strategy_insights: { success_score: 82, posting_time: en ? "8:30 PM" : "8:30 مساء", reasoning: en ? "Uses approved Vision and new-account baseline." : "يعتمد على وصف Vision ومسار الحساب الجديد." },
          virality_indicators: { hook_strength: "Medium", shareability_factor: "High", predicted_trend_alignment: "Rising", strategic_score: 78 }
        }
      ],
      rationale: [en ? "No invented metrics were used." : "لم يتم اختراع أرقام."],
      data_points: [en ? "Vision description approved." : "وصف Vision معتمد."],
      guardrails: [en ? "Manual publishing only." : "النشر يدوي فقط."]
    };
  }
  function report(language, text) {
    const en = language === "en";
    if (en) {
      return "## Ready Caption\\n" + (text || enCaption) + "\\n\\n## Hashtags\\n" + tags.slice(0, 8).join(" ") + "\\n\\n## Suggested Posting Time\\n8:30 PM\\n\\n## Notes Before Publishing\\n- Check the first two seconds.\\n- Paste manually inside the platform.";
    }
    return "## الكابشن الجاهز\\n" + (text || arCaption) + "\\n\\n## الهاشتاقات\\n" + tags.slice(0, 8).join(" ") + "\\n\\n## وقت النشر المقترح\\n8:30 مساء\\n\\n## ملاحظات للعميل قبل النشر\\n- راجع أول ثانيتين.\\n- الصق النص يدوياً داخل المنصة.";
  }
  const callbacks = {};
  let callbackId = 1;
  window.__E2E_CALLS__ = calls;
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener() {}
  };
  window.__TAURI_INTERNALS__ = {
    callbacks,
    transformCallback(callback, once) {
      const id = callbackId++;
      callbacks[id] = callback;
      window["_" + id] = (payload) => {
        callback(payload);
        if (once) delete callbacks[id];
      };
      return id;
    },
    unregisterCallback(id) {
      delete callbacks[id];
      delete window["_" + id];
    },
    convertFileSrc(path) {
      return path;
    },
    async invoke(cmd, args = {}) {
      calls.push({ cmd, args });
      if (cmd === "plugin:event|listen") return calls.length;
      if (cmd === "plugin:event|unlisten") return null;
      if (cmd === "plugin:dialog|open") return ${JSON.stringify(media)};
      if (cmd === "plugin:opener|open_url") return null;
      if (cmd === "license_status") return { active: true, message: "E2E active license", expires_at: "2026-12-31T00:00:00Z", days_remaining: 230, renewal_warning: false, renewal_phone: "+96500000000", features: ["all"] };
      if (cmd === "app_status") return { model_installed: true, server_running: false, model_path: "C:/Users/FARHAN/AppData/Roaming/GrowBoxPro/models/hermes-2-pro-llama-3-8b-q4km.gguf" };
      if (cmd === "system_preflight") return { ram_gb: 32, vram_gb: 8, avx2: true, cuda: false, metal: false, recommended_profile: "balanced-q4", vision_supported: true, warnings: [] };
      if (cmd === "connected_accounts") return [];
      if (cmd === "list_clients") return [{ id: 1, client_name: "E2E Client", generated_at: now, status: "done", content: report("ar") }];
      if (cmd === "background_status") return { active: backgroundActive, last_run_at: backgroundActive ? now : null, last_summary: backgroundActive ? "Background active." : "Background stopped.", next_check_minutes: 360 };
      if (cmd === "evolution_status") return { active: evolutionActive, last_run_at: evolutionActive ? now : null, last_summary: evolutionActive ? "Evolution active." : "Evolution stopped.", next_check_days: 7 };
      if (cmd === "list_evolution_reports") return [];
      if (cmd === "list_login_credentials") return loginSaved ? [{ id: "cred-1", platform: "instagram", username: "e2e_user", profile_label: "instagram-e2e_user", created_at: now, updated_at: now }] : [];
      if (cmd === "vision_status") return { ready: visionReady, model_path: "qwen2-vl-2b-instruct.gguf", mmproj_path: "mmproj-qwen2-vl-2b-instruct.gguf", model_hint: "download once", model_size: visionReady ? 2500000000 : 0, mmproj_size: visionReady ? 500000000 : 0 };
      if (cmd === "setup_app") return null;
      if (cmd === "download_vision_model") { visionReady = true; return { ready: true, model_path: "qwen2-vl-2b-instruct.gguf", mmproj_path: "mmproj-qwen2-vl-2b-instruct.gguf", model_hint: "ready", model_size: 2500000000, mmproj_size: 500000000 }; }
      if (cmd === "launch_isolated_browser" || cmd === "launch_official_view" || cmd === "launch_publish_page") { browserRunning = true; return { running: true, port: args.platform === "tiktok" ? 9223 : 9222, browser: "Chrome", profile_dir: "mock-profile", profile_label: args.accountLabel || "default" }; }
      if (cmd === "isolated_browser_status") return { running: browserRunning, port: 9222, browser: browserRunning ? "Chrome" : null, profile_dir: "mock-profile", profile_label: "instagram-e2e" };
      if (cmd === "stop_isolated_browser") { browserRunning = false; return { running: false, port: 9222, browser: null, profile_dir: "mock-profile", profile_label: "instagram-e2e" }; }
      if (cmd === "save_login_credential") { loginSaved = true; return null; }
      if (cmd === "delete_login_credential") { loginSaved = false; return null; }
      if (cmd === "open_saved_session" || cmd === "launch_saved_account_session") { browserRunning = true; return { running: true, port: 9222, browser: "Chrome", profile_dir: "mock-profile", profile_label: "instagram-e2e_user" }; }
      if (cmd === "start_oauth") return "https://example.com/oauth";
      if (cmd === "complete_oauth") return null;
      if (cmd === "inspect_media_with_vision") return { media_kind: "video", description: "## Vision Agent Description\\nFrame 1: soft baby reception setup.\\nFrame 2: pink and gold details.\\nFrame 3: clear decorative hook.", source_note: "Read 3 video frames with Qwen2-VL.", warning: null };
      if (cmd === "fetch_post_insights") return { platform: args.platform || "instagram", post_url: args.postUrl || "mock", captured_at: now, raw_endpoints: [{ url: "mock-json" }], normalized: { views: 12000, likes: 820, comments: 31, shares: 55, saves: 144, engagement_rate: 8.75 }, discovered_urls: [] };
      if (cmd === "discover_competitor_posts") return ["https://www.instagram.com/reel/mock1/", "https://www.instagram.com/reel/mock2/"];
      if (cmd === "analyze_with_market_evidence" || cmd === "analyze_publish_strategy" || cmd === "analyze_with_fetched_insights") return output(args.input?.output_language || "ar");
      if (cmd === "generate_post_draft") return { id: ++reportId, client_name: args.input?.client?.name || "E2E Client", generated_at: now, status: "done", content: report(args.input?.output_language || "ar", args.input?.approved_publish_text || "") };
      if (cmd === "load_report") return report("ar");
      if (cmd === "start_evolution_tracker") { evolutionActive = true; return { active: true, last_run_at: now, last_summary: "Evolution tracker started.", next_check_days: 7 }; }
      if (cmd === "stop_evolution_tracker") { evolutionActive = false; return { active: false, last_run_at: null, last_summary: "Evolution tracker stopped.", next_check_days: 7 }; }
      if (cmd === "run_evolution_report" || cmd === "run_evolution_report_now") return { id: 2, generated_at: now, content: "## Strategic Health Report\\nNo deviation detected in mock E2E." };
      if (cmd === "start_background_mode") { backgroundActive = true; return { active: true, last_run_at: now, last_summary: "Background mode started.", next_check_minutes: 360 }; }
      if (cmd === "stop_background_mode") { backgroundActive = false; return { active: false, last_run_at: null, last_summary: "Background mode stopped.", next_check_minutes: 360 }; }
      if (cmd === "minimize_to_tray") return null;
      if (cmd === "activate_license") return { active: true, message: "Activated", expires_at: "2026-12-31T00:00:00Z", days_remaining: 230, renewal_warning: false, renewal_phone: "+96500000000", features: ["all"] };
      throw new Error("Unmocked Tauri command: " + cmd);
    }
  };
  navigator.clipboard = { writeText: async (text) => { window.__E2E_CLIPBOARD__ = text; } };
})();
`;
}

async function clickText(page, text, index = 0) {
  const locator = page.locator("button", { hasText: text }).nth(index);
  await locator.scrollIntoViewIfNeeded();
  await locator.click();
}

async function main() {
  const vite = process.platform === "win32"
    ? spawn("cmd.exe", ["/d", "/s", "/c", "npm run dev -- --host 127.0.0.1 --port 1420"], {
        cwd: root,
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
      })
    : spawn("npm", ["run", "dev", "--", "--host", "127.0.0.1", "--port", "1420"], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
      });
  let browser;
  try {
    await waitForServer("http://127.0.0.1:1420");
    browser = await chromium.launch({ executablePath: chromePath, headless: true });
    const page = await browser.newPage({ viewport: { width: 1366, height: 1100 } });
    const consoleErrors = [];
    page.on("console", (msg) => {
      if (["error", "warning"].includes(msg.type())) consoleErrors.push(`${msg.type()}: ${msg.text()}`);
    });
    page.on("pageerror", (error) => consoleErrors.push(`pageerror: ${error.message}`));
    await page.addInitScript(tauriMock(mediaPath));

    await step("Load app shell", async () => {
      await page.goto("http://127.0.0.1:1420", { waitUntil: "networkidle" });
      await page.locator("h1", { hasText: "GrowBox Pro" }).waitFor({ timeout: 10_000 });
    });

    await step("Language switch changes direction and keeps English workflow", async () => {
      await clickText(page, "English");
      await page.waitForFunction(() => document.querySelector(".min-h-screen")?.getAttribute("dir") === "ltr");
      await clickText(page, "العربية");
      await page.waitForFunction(() => document.querySelector(".min-h-screen")?.getAttribute("dir") === "rtl");
      await clickText(page, "English");
    });

    await step("Setup buttons", async () => {
      await clickText(page, "إعادة الفحص");
      await clickText(page, "تحميل Qwen2-VL");
      await page.locator("text=جاهز لتحليل الوسائط").waitFor({ timeout: 5_000 });
    });

    await step("Account/session/vault/OAuth controls", async () => {
      await page.locator("#cn").fill("E2E Client");
      await page.locator("#ig").fill("@e2e_instagram");
      await page.locator("#tt").fill("@e2e_tiktok");
      await page.locator("#nc").selectOption("تصوير");
      await clickText(page, "فتح الجلسة");
      await page.locator("text=تعمل على").waitFor({ timeout: 5_000 });
      await clickText(page, "إغلاق");
      await page.locator("summary", { hasText: "إدارة بيانات الدخول" }).click();
      await page.locator("input[placeholder='@username']").first().fill("e2e_user");
      await page.locator("input[type='password']").fill("secret");
      await clickText(page, "حفظ", 0);
      await page.locator("text=@e2e_user").waitFor({ timeout: 5_000 });
      await clickText(page, "فتح", 0);
      await page.locator("input[placeholder='OAuth code']").fill("oauth-code");
      await clickText(page, "ربط");
      await clickText(page, "حفظ", 1);
    });

    await step("Vision media selection, inspection, and approval", async () => {
      await clickText(page, "اختيار ملف");
      await page.locator("#media").waitFor({ timeout: 5_000 });
      await page.locator("#media").evaluate((el) => {
        if (!el.value) throw new Error("Media path was not populated");
      });
      await clickText(page, "قراءة الوسائط");
      await page.locator("#vision-note").waitFor({ timeout: 5_000 });
      await page.locator("#vision-note").evaluate((el) => {
        if (!el.value.includes("Frame 1")) throw new Error("Vision description missing");
      });
      await clickText(page, "موافقة");
      await page.locator("text=تم اعتماد الوصف").waitFor({ timeout: 5_000 });
    });

    await step("New-account evidence path and market analysis", async () => {
      await page.locator("input[type='checkbox']").check();
      await clickText(page, "موافق: ابدأ من الصفر");
      await clickText(page, "جلب وتحليل بالأدلة");
      await page.locator("text=Growth Score").waitFor({ timeout: 10_000 });
      await page.locator("text=A polished baby reception").waitFor({ timeout: 5_000 });
      const hashtagCount = await page.locator(".preview-card .tag-cloud .chip").count();
      if (hashtagCount > 8) throw new Error(`Expected <= 8 hashtags, got ${hashtagCount}`);
    });

    await step("Approve card, generate English draft, copy/download/open publish page", async () => {
      await clickText(page, "اعتماد هذه النسخة");
      await page.locator("text=Ready Caption").waitFor({ timeout: 10_000 });
      await page.locator("text=المسودة الجاهزة").waitFor({ timeout: 5_000 });
      await clickText(page, "نسخ");
      await clickText(page, "تنزيل");
      await clickText(page, "فتح صفحة النشر");
      await page.locator("text=تم فتح صفحة نشر").waitFor({ timeout: 5_000 });
    });

    await step("Evolution tracker and background tools", async () => {
      await clickText(page, "تشغيل التتبع");
      await page.locator("text=التتبع يعمل").waitFor({ timeout: 5_000 });
      await clickText(page, "تقرير الآن");
      await page.locator("text=Strategic Health Report").waitFor({ timeout: 5_000 });
      await page.locator("summary", { hasText: "الأدوات المتقدمة" }).click();
      await clickText(page, "تشغيل الخلفية");
      await clickText(page, "تصغير للشريط");
      await page.locator("text=Background mode started.").waitFor({ timeout: 5_000 });
    });

    await step("No unhandled console/page errors", async () => {
      const critical = consoleErrors.filter((msg) => !msg.includes("favicon"));
      if (critical.length) throw new Error(critical.join("\\n"));
    });

    calls.push(...await page.evaluate(() => window.__E2E_CALLS__ || []));
    await page.screenshot({ path: join(desktop, "GrowBox-Pro-E2E-UI-final.png"), fullPage: true });
  } finally {
    if (browser) await browser.close();
    if (process.platform === "win32") {
      spawnSync("taskkill.exe", ["/pid", String(vite.pid), "/T", "/F"], { stdio: "ignore" });
    } else {
      vite.kill();
    }
  }

  const passed = results.filter((r) => r.status === "PASS").length;
  const failed = results.filter((r) => r.status === "FAIL").length;
  const uniqueCommands = [...new Set(calls.map((call) => call.cmd))].sort();
  const body = [
    "# GrowBox Pro E2E UI Smoke Results",
    "",
    `- Passed: ${passed}`,
    `- Failed: ${failed}`,
    `- Tauri commands exercised: ${uniqueCommands.length}`,
    "",
    "## Steps",
    ...results.map((r) => `- ${r.status}: ${r.name}${r.details ? `\\n  ${r.details.replace(/\\n/g, "\\n  ")}` : ""}`),
    "",
    "## Commands",
    ...uniqueCommands.map((cmd) => `- ${cmd}`),
    "",
  ].join("\\n");
  writeFileSync(reportPath, body, "utf8");
  console.log(body);
  console.log(`REPORT_PATH=${reportPath}`);
  if (failed) process.exit(1);
}

main().catch((error) => {
  fail("E2E runner", error);
  const body = [
    "# GrowBox Pro E2E UI Smoke Results",
    "",
    ...results.map((r) => `- ${r.status}: ${r.name}${r.details ? `\\n  ${r.details.replace(/\\n/g, "\\n  ")}` : ""}`),
    "",
    `Fatal: ${String(error?.stack || error)}`,
  ].join("\\n");
  writeFileSync(reportPath, body, "utf8");
  console.error(body);
  console.error(`REPORT_PATH=${reportPath}`);
  process.exit(1);
});
