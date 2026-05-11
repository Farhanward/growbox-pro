<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { marked } from "marked";
  import Icon from "$lib/Icon.svelte";

  type Status = { model_installed: boolean; server_running: boolean; model_path: string };
  type Report = { id: number; client_name: string; generated_at: string; status: string; content: string };

  const COUNTRIES = [
    { code: "SA", label: "السعودية", flag: "🇸🇦" },
    { code: "AE", label: "الإمارات", flag: "🇦🇪" },
    { code: "KW", label: "الكويت", flag: "🇰🇼" },
    { code: "QA", label: "قطر", flag: "🇶🇦" },
    { code: "BH", label: "البحرين", flag: "🇧🇭" },
    { code: "OM", label: "عُمان", flag: "🇴🇲" },
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
  let viewingId = $state<number | null>(null);
  let viewingContent = $state("");
  let history = $state<Report[]>([]);
  let formError = $state("");

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
        stageMsg = stage === "model" ? "تحميل النموذج" :
                   stage === "server" ? "تشغيل المحرّك" :
                   stage === "ready" ? "جاهز" : "";
      },
    );
    await listen<{ stage: string; message: string }>("report:progress", (e) => {
      progressMsg = e.payload.message;
    });
  });

  async function refreshStatus() { try { status = await invoke<Status>("app_status"); } catch {} }
  async function refreshHistory() { try { history = await invoke<Report[]>("list_clients"); } catch {} }

  async function runSetup() {
    setupRunning = true; setupError = ""; stage = "model"; percent = 0;
    try {
      await invoke("setup_app");
      await refreshStatus();
      stage = "ready";
    } catch (e) { setupError = String(e); stage = "error"; }
    finally { setupRunning = false; }
  }

  function toggleCountry(code: string) {
    countries = countries.includes(code) ? countries.filter(c => c !== code) : [...countries, code];
  }

  async function generate() {
    if (!name.trim()) { formError = "أدخل اسم العميل."; return; }
    working = true; formError = ""; progressMsg = "تجهيز…";
    try {
      const r = await invoke<Report>("generate_report", {
        input: {
          name: name.trim(),
          tiktok: tiktok.trim() || null,
          instagram: instagram.trim() || null,
          snapchat: snapchat.trim() || null,
          niche, countries, plan_days: planDays,
        },
      });
      viewingId = r.id; viewingContent = r.content;
      await refreshHistory();
    } catch (e) { formError = `${e}`; }
    finally { working = false; progressMsg = ""; }
  }

  async function viewReport(id: number) {
    try {
      viewingContent = await invoke<string>("load_report", { id });
      viewingId = id;
    } catch (e) { formError = `${e}`; }
  }

  function downloadMarkdown() {
    if (!viewingContent) return;
    const blob = new Blob([viewingContent], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url; a.download = `report-${viewingId}.md`; a.click();
    URL.revokeObjectURL(url);
  }

  let isReady = $derived(status?.model_installed && status?.server_running);
  let rendered = $derived(viewingContent ? marked.parse(viewingContent, { breaks: true }) : "");
</script>

<div class="min-h-screen">
  <!-- Top bar with vibrancy -->
  <header class="vibrancy sticky top-0 z-10 px-8 py-4 flex items-center justify-between">
    <div class="flex items-center gap-3">
      <div class="w-10 h-10 rounded-2xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-white">
        <Icon name="sparkle" size={22} />
      </div>
      <div>
        <h1 class="text-[17px] font-semibold leading-tight">Reach Optimizer</h1>
        <p class="text-[12px]" style="color: var(--text-2)">تحليل خليجي عميق · بدون تسجيل دخول</p>
      </div>
    </div>
    {#if status}
      <span class="pill">
        <span class="w-1.5 h-1.5 rounded-full" style="background: {isReady ? 'var(--success)' : 'var(--warning)'}"></span>
        {isReady ? "جاهز" : "يحتاج تجهيز"}
      </span>
    {/if}
  </header>

  <main class="max-w-4xl mx-auto px-8 py-10 space-y-6">
    {#if !isReady}
      <div class="surface px-10 py-14 text-center">
        <div class="w-16 h-16 mx-auto mb-6 rounded-[22px] bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-white shadow-lg">
          <Icon name="sparkle" size={32} />
        </div>
        <h2 class="text-[22px] font-semibold mb-2">مرحباً بك</h2>
        <p class="max-w-md mx-auto mb-8" style="color: var(--text-2)">
          نحتاج تحميل نموذج الذكاء الاصطناعي مرة واحدة (~٥.٤ جيجا). كل شيء يجري تلقائياً.
        </p>

        {#if !setupRunning && stage !== "ready"}
          <button class="btn btn-primary" onclick={runSetup}>
            <Icon name="play" size={18} />
            ابدأ التجهيز
          </button>
        {/if}

        {#if setupRunning}
          <div class="max-w-sm mx-auto">
            <div class="flex items-center justify-center gap-2 mb-4">
              <Icon name="spinner" size={18} class="animate-spin" />
              <span class="text-[15px] font-medium" style="color: var(--primary)">{stageMsg}</span>
            </div>
            <div class="progress-track">
              <div class="progress-fill" style="width: {percent}%"></div>
            </div>
            <p class="text-[12px] mt-3" style="color: var(--text-2)">
              {percent}%{stage === "model" && totalMb ? ` · ${downloadedMb} من ${totalMb} ميجا` : ""}
            </p>
          </div>
        {/if}

        {#if setupError}
          <p class="text-[13px] mt-6 max-w-md mx-auto" style="color: var(--danger)">{setupError}</p>
          <button class="btn btn-ghost mt-4" onclick={runSetup}>
            <Icon name="refresh" size={16} />
            إعادة المحاولة
          </button>
        {/if}
      </div>
    {:else}
      <!-- Client form -->
      <div class="surface p-8">
        <div class="flex items-center gap-3 mb-6">
          <div class="w-9 h-9 rounded-xl flex items-center justify-center" style="background: var(--surface-2); color: var(--text)">
            <Icon name="user" size={20} />
          </div>
          <h2 class="text-[18px] font-semibold">بيانات العميل</h2>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-5">
          <div class="md:col-span-2">
            <label class="field-label" for="cn">اسم العميل</label>
            <input id="cn" class="field-input" bind:value={name} placeholder="مثال: فاطمة لينس" />
          </div>
          <div>
            <label class="field-label" for="tt"><span class="inline-flex items-center gap-1.5"><Icon name="tiktok" size={14} /> TikTok</span></label>
            <input id="tt" class="field-input" bind:value={tiktok} placeholder="@username" />
          </div>
          <div>
            <label class="field-label" for="ig"><span class="inline-flex items-center gap-1.5"><Icon name="instagram" size={14} /> Instagram</span></label>
            <input id="ig" class="field-input" bind:value={instagram} placeholder="@username" />
          </div>
          <div>
            <label class="field-label" for="sc"><span class="inline-flex items-center gap-1.5"><Icon name="snapchat" size={14} /> Snapchat</span></label>
            <input id="sc" class="field-input" bind:value={snapchat} placeholder="username" />
          </div>
          <div>
            <label class="field-label" for="nc"><span class="inline-flex items-center gap-1.5"><Icon name="chart" size={14} /> المجال</span></label>
            <select id="nc" class="field-input" bind:value={niche}>
              {#each NICHES as n}<option value={n}>{n}</option>{/each}
            </select>
          </div>
          <div class="md:col-span-2">
            <span class="field-label"><span class="inline-flex items-center gap-1.5"><Icon name="globe" size={14} /> الدول المستهدفة</span></span>
            <div class="flex flex-wrap gap-2">
              {#each COUNTRIES as c}
                <button type="button" onclick={() => toggleCountry(c.code)}
                        class="chip {countries.includes(c.code) ? 'chip-on' : ''}">
                  <span>{c.flag}</span>{c.label}
                </button>
              {/each}
            </div>
          </div>
          <div>
            <label class="field-label" for="pd"><span class="inline-flex items-center gap-1.5"><Icon name="calendar" size={14} /> مدة الخطة</span></label>
            <select id="pd" class="field-input" bind:value={planDays}>
              {#each [7, 14, 30, 60, 90] as d}<option value={d}>{d} يوم</option>{/each}
            </select>
          </div>
        </div>

        <div class="mt-8 flex items-center gap-3">
          <button class="btn btn-primary" disabled={working} onclick={generate}>
            {#if working}
              <Icon name="spinner" size={18} class="animate-spin" /> جارٍ التحليل
            {:else}
              <Icon name="sparkle" size={18} /> تحليل وتوليد التقرير
            {/if}
          </button>
          {#if working && progressMsg}
            <span class="text-[13px]" style="color: var(--text-2)">{progressMsg}</span>
          {/if}
          {#if formError}
            <span class="text-[13px]" style="color: var(--danger)">{formError}</span>
          {/if}
        </div>
      </div>

      {#if viewingContent}
        <div class="surface p-8">
          <div class="flex items-center justify-between mb-5">
            <h2 class="text-[18px] font-semibold">التقرير #{viewingId}</h2>
            <div class="flex gap-2">
              <button class="btn btn-ghost" onclick={downloadMarkdown} style="min-height:36px;padding:0 14px;font-size:13px">
                <Icon name="download" size={16} /> تنزيل
              </button>
              <button class="btn btn-ghost" onclick={() => { viewingContent = ""; viewingId = null; }} style="min-height:36px;padding:0 14px;font-size:13px">
                <Icon name="x" size={16} /> إغلاق
              </button>
            </div>
          </div>
          <div class="divider mb-5"></div>
          <div class="report-body">{@html rendered}</div>
        </div>
      {/if}

      <div class="surface p-8">
        <div class="flex items-center gap-3 mb-6">
          <div class="w-9 h-9 rounded-xl flex items-center justify-center" style="background: var(--surface-2)">
            <Icon name="chart" size={20} />
          </div>
          <h2 class="text-[18px] font-semibold">العملاء السابقون</h2>
        </div>
        {#if history.length === 0}
          <p class="text-[14px]" style="color: var(--text-2)">لا يوجد عملاء بعد. ابدأ بتحليل أول حساب من النموذج أعلاه.</p>
        {:else}
          <div class="space-y-0">
            {#each history as h, i}
              {#if i > 0}<div class="divider"></div>{/if}
              <button onclick={() => viewReport(h.id)}
                      class="w-full flex items-center justify-between py-3.5 text-right hover:bg-[var(--surface-2)] rounded-xl px-2 transition">
                <span class="font-medium">{h.client_name}</span>
                <span class="text-[12px]" style="color: var(--text-2)">
                  {new Date(h.generated_at).toLocaleDateString("ar")}
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </main>
</div>
