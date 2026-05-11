<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { marked } from "marked";

  type Status = { model_installed: boolean; server_running: boolean; model_path: string };
  type Report = { id: number; client_name: string; generated_at: string; status: string; content: string };

  const COUNTRIES = [
    { code: "SA", label: "السعودية" },
    { code: "AE", label: "الإمارات" },
    { code: "KW", label: "الكويت" },
    { code: "QA", label: "قطر" },
    { code: "BH", label: "البحرين" },
    { code: "OM", label: "عُمان" },
  ];
  const NICHES = ["تصوير", "موضة", "طعام", "سفر", "تجميل", "رياضة", "تعليم", "ألعاب"];

  let status = $state<Status | null>(null);
  let setupRunning = $state(false);
  let stage = $state<"idle" | "model" | "server" | "ready" | "error">("idle");
  let stageMsg = $state("");
  let percent = $state(0);
  let downloadedMb = $state(0);
  let totalMb = $state(0);
  let setupError = $state("");

  let name = $state("");
  let tiktok = $state("");
  let instagram = $state("");
  let snapchat = $state("");
  let niche = $state(NICHES[0]);
  let countries = $state<string[]>(["SA", "KW"]);
  let planDays = $state(30);

  let working = $state(false);
  let progressMsg = $state("");
  let lastReport = $state<Report | null>(null);
  let history = $state<Report[]>([]);
  let formError = $state("");
  let viewingId = $state<number | null>(null);
  let viewingContent = $state("");

  onMount(async () => {
    await refreshStatus();
    await refreshHistory();
    await listen<{ stage: string; percent: number; downloaded?: number; total?: number }>(
      "setup:progress",
      (e) => {
        stage = e.payload.stage as any;
        percent = Math.round(e.payload.percent ?? 0);
        if (e.payload.downloaded != null) downloadedMb = Math.round(e.payload.downloaded / 1_048_576);
        if (e.payload.total != null) totalMb = Math.round(e.payload.total / 1_048_576);
        stageMsg = stage === "model" ? "جاري تحميل النموذج…"
                : stage === "server" ? "جاري تشغيل المحرّك…"
                : stage === "ready" ? "جاهز" : "";
      },
    );
    await listen<{ stage: string; message: string }>("report:progress", (e) => {
      progressMsg = e.payload.message;
    });
  });

  async function refreshStatus() {
    try { status = await invoke<Status>("app_status"); } catch {}
  }
  async function refreshHistory() {
    try { history = await invoke<Report[]>("list_clients"); } catch {}
  }
  async function runSetup() {
    setupRunning = true;
    setupError = "";
    stage = "model";
    percent = 0;
    try {
      await invoke("setup_app");
      await refreshStatus();
      stage = "ready";
    } catch (e) {
      setupError = String(e);
      stage = "error";
    } finally {
      setupRunning = false;
    }
  }

  function toggleCountry(code: string) {
    countries = countries.includes(code) ? countries.filter(c => c !== code) : [...countries, code];
  }

  async function generate() {
    if (!name.trim()) { formError = "أدخل اسم العميل."; return; }
    working = true;
    formError = "";
    progressMsg = "تجهيز…";
    try {
      lastReport = await invoke<Report>("generate_report", {
        input: {
          name: name.trim(),
          tiktok: tiktok.trim() || null,
          instagram: instagram.trim() || null,
          snapchat: snapchat.trim() || null,
          niche, countries, plan_days: planDays,
        },
      });
      viewingId = lastReport.id;
      viewingContent = lastReport.content;
      await refreshHistory();
    } catch (e) {
      formError = `${e}`;
    } finally {
      working = false;
      progressMsg = "";
    }
  }

  async function viewReport(id: number) {
    try {
      const content = await invoke<string>("load_report", { id });
      viewingId = id;
      viewingContent = content;
    } catch (e) {
      formError = `${e}`;
    }
  }

  function downloadMarkdown() {
    if (!viewingContent) return;
    const blob = new Blob([viewingContent], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `report-${viewingId}.md`;
    a.click();
    URL.revokeObjectURL(url);
  }

  let isReady = $derived(status?.model_installed && status?.server_running);
  let rendered = $derived(viewingContent ? marked.parse(viewingContent, { breaks: true }) : "");
</script>

<div class="min-h-screen px-6 py-8 max-w-5xl mx-auto">
  <header class="flex items-center justify-between mb-8">
    <div>
      <h1 class="text-2xl font-bold tracking-tight">Reach Optimizer</h1>
      <p class="text-sm text-zinc-400 mt-1">تحليل عميق للحسابات الخليجية — بدون تسجيل دخول</p>
    </div>
    {#if status}
      <span class="pill">
        <span class="w-2 h-2 rounded-full {isReady ? 'bg-emerald-400' : 'bg-amber-400'}"></span>
        {isReady ? "جاهز" : "يحتاج تجهيز"}
      </span>
    {/if}
  </header>

  {#if !isReady}
    <div class="card text-center py-12">
      <h2 class="text-xl font-semibold mb-3">مرحباً بك 👋</h2>
      <p class="text-zinc-400 mb-8 max-w-md mx-auto">
        قبل الاستخدام، نحتاج تحميل نموذج الذكاء الاصطناعي مرة واحدة (~5.8 جيجا). اضغط الزر وانتظر.
      </p>
      {#if !setupRunning && stage !== "ready"}
        <button class="btn-primary text-lg px-8 py-3" onclick={runSetup}>ابدأ التجهيز</button>
      {/if}
      {#if setupRunning}
        <div class="max-w-md mx-auto">
          <p class="text-emerald-400 font-medium mb-3">{stageMsg}</p>
          <div class="w-full bg-zinc-800 rounded-full h-3 overflow-hidden mb-2">
            <div class="bg-emerald-500 h-full transition-all" style="width: {percent}%"></div>
          </div>
          <p class="text-xs text-zinc-500">
            {percent}%{stage === "model" && totalMb ? ` — ${downloadedMb} / ${totalMb} ميجا` : ""}
          </p>
        </div>
      {/if}
      {#if setupError}
        <p class="text-red-400 text-sm mt-6 max-w-md mx-auto">{setupError}</p>
        <button class="btn-ghost mt-3" onclick={runSetup}>إعادة المحاولة</button>
      {/if}
    </div>
  {:else}
    <div class="card mb-6">
      <h2 class="font-semibold mb-4">حساب العميل</h2>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div class="md:col-span-2">
          <label class="field-label" for="cn">اسم العميل</label>
          <input id="cn" class="field-input" bind:value={name} placeholder="فاطمة لينس" />
        </div>
        <div>
          <label class="field-label" for="tt">TikTok</label>
          <input id="tt" class="field-input" bind:value={tiktok} placeholder="@username" />
        </div>
        <div>
          <label class="field-label" for="ig">Instagram</label>
          <input id="ig" class="field-input" bind:value={instagram} placeholder="@username" />
        </div>
        <div>
          <label class="field-label" for="sc">Snapchat</label>
          <input id="sc" class="field-input" bind:value={snapchat} placeholder="username" />
        </div>
        <div>
          <label class="field-label" for="nc">المجال</label>
          <select id="nc" class="field-input" bind:value={niche}>
            {#each NICHES as n}<option value={n}>{n}</option>{/each}
          </select>
        </div>
        <div class="md:col-span-2">
          <span class="field-label">الدول المستهدفة</span>
          <div class="flex flex-wrap gap-2">
            {#each COUNTRIES as c}
              <button type="button" onclick={() => toggleCountry(c.code)}
                class="px-3 py-1.5 rounded-lg text-sm border transition
                  {countries.includes(c.code)
                    ? 'bg-emerald-500/20 border-emerald-500 text-emerald-300'
                    : 'bg-zinc-800/70 border-zinc-700 text-zinc-300 hover:border-zinc-500'}">
                {c.label}
              </button>
            {/each}
          </div>
        </div>
        <div>
          <label class="field-label" for="pd">مدة الخطة (يوم)</label>
          <select id="pd" class="field-input" bind:value={planDays}>
            {#each [7, 14, 30, 60, 90] as d}<option value={d}>{d} يوم</option>{/each}
          </select>
        </div>
      </div>
      <div class="mt-6 flex items-center gap-3">
        <button class="btn-primary" disabled={working} onclick={generate}>
          {working ? "جارٍ التحليل…" : "تحليل وتوليد التقرير"}
        </button>
        {#if working && progressMsg}
          <span class="text-sm text-zinc-400">{progressMsg}</span>
        {/if}
        {#if formError}<span class="text-sm text-red-400">{formError}</span>{/if}
      </div>
    </div>

    {#if viewingContent}
      <div class="card mb-6">
        <div class="flex items-center justify-between mb-4">
          <h2 class="font-semibold">التقرير #{viewingId}</h2>
          <div class="flex gap-2">
            <button class="btn-ghost text-sm" onclick={downloadMarkdown}>تنزيل Markdown</button>
            <button class="btn-ghost text-sm" onclick={() => { viewingContent = ""; viewingId = null; }}>إغلاق</button>
          </div>
        </div>
        <div class="prose-report">
          {@html rendered}
        </div>
      </div>
    {/if}

    <div class="card">
      <h2 class="font-semibold mb-4">العملاء السابقون</h2>
      {#if history.length === 0}
        <p class="text-sm text-zinc-500">لا يوجد عملاء بعد.</p>
      {:else}
        <ul class="divide-y divide-zinc-800">
          {#each history as h}
            <li class="py-3 flex items-center justify-between">
              <button class="text-right hover:text-emerald-400 transition" onclick={() => viewReport(h.id)}>
                {h.client_name}
              </button>
              <span class="text-xs text-zinc-500">
                {new Date(h.generated_at).toLocaleDateString("ar")}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>

<style>
  :global(.prose-report) {
    color: #e4e4e7;
    line-height: 1.8;
  }
  :global(.prose-report h1),
  :global(.prose-report h2),
  :global(.prose-report h3) {
    color: #34d399;
    font-weight: 700;
    margin-top: 1.2em;
    margin-bottom: 0.5em;
  }
  :global(.prose-report h1) { font-size: 1.5rem; }
  :global(.prose-report h2) { font-size: 1.25rem; }
  :global(.prose-report h3) { font-size: 1.1rem; }
  :global(.prose-report ul),
  :global(.prose-report ol) {
    padding-right: 1.5em;
    margin: 0.5em 0;
  }
  :global(.prose-report li) { margin: 0.25em 0; }
  :global(.prose-report p) { margin: 0.5em 0; }
  :global(.prose-report strong) { color: #fbbf24; }
  :global(.prose-report code) {
    background: #18181b;
    padding: 0.15em 0.35em;
    border-radius: 4px;
    font-size: 0.9em;
  }
</style>
