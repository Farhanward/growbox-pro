<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type Status = { installed: boolean; path: string; size_bytes: number };
  type Report = { id: number; client_name: string; generated_at: string; status: string };

  const COUNTRIES = [
    { code: "SA", label: "السعودية" },
    { code: "AE", label: "الإمارات" },
    { code: "KW", label: "الكويت" },
    { code: "QA", label: "قطر" },
    { code: "BH", label: "البحرين" },
    { code: "OM", label: "عُمان" },
  ];
  const NICHES = ["تصوير", "موضة", "طعام", "سفر", "تجميل", "رياضة", "تعليم", "ألعاب"];

  let modelStatus = $state<Status | null>(null);
  let downloading = $state(false);
  let downloadPct = $state(0);
  let downloadedMb = $state(0);
  let totalMb = $state(0);

  let name = $state("");
  let tiktok = $state("");
  let instagram = $state("");
  let snapchat = $state("");
  let niche = $state(NICHES[0]);
  let countries = $state<string[]>(["SA", "KW"]);
  let planDays = $state(30);

  let working = $state(false);
  let lastReport = $state<Report | null>(null);
  let history = $state<Report[]>([]);
  let errorMsg = $state("");

  onMount(async () => {
    await refreshStatus();
    await refreshHistory();
    await listen<{ percent: number; downloaded: number; total: number }>(
      "model:progress",
      (e) => {
        downloadPct = Math.round(e.payload.percent);
        downloadedMb = Math.round(e.payload.downloaded / 1_048_576);
        totalMb = Math.round(e.payload.total / 1_048_576);
      },
    );
  });

  async function refreshStatus() {
    try {
      modelStatus = await invoke<Status>("check_model_status");
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function refreshHistory() {
    try {
      history = await invoke<Report[]>("list_clients");
    } catch (e) {
      // fine if DB empty
    }
  }

  async function startDownload() {
    downloading = true;
    errorMsg = "";
    try {
      await invoke("download_model");
      await refreshStatus();
    } catch (e) {
      errorMsg = `فشل التحميل: ${e}`;
    } finally {
      downloading = false;
    }
  }

  function toggleCountry(code: string) {
    countries = countries.includes(code)
      ? countries.filter((c) => c !== code)
      : [...countries, code];
  }

  async function generate() {
    if (!modelStatus?.installed) {
      errorMsg = "النموذج غير مُحمَّل بعد.";
      return;
    }
    if (!name.trim()) {
      errorMsg = "أدخل اسم العميل.";
      return;
    }
    working = true;
    errorMsg = "";
    try {
      lastReport = await invoke<Report>("generate_plan", {
        input: {
          name: name.trim(),
          tiktok: tiktok.trim() || null,
          instagram: instagram.trim() || null,
          snapchat: snapchat.trim() || null,
          niche,
          countries,
          plan_days: planDays,
        },
      });
      await refreshHistory();
    } catch (e) {
      errorMsg = `تعذّر توليد الخطة: ${e}`;
    } finally {
      working = false;
    }
  }
</script>

<div class="min-h-screen px-6 py-8 max-w-5xl mx-auto">
  <header class="flex items-center justify-between mb-8">
    <div>
      <h1 class="text-2xl font-bold tracking-tight">Reach Optimizer</h1>
      <p class="text-sm text-zinc-400 mt-1">خطة انتشار خليجية بدون تسجيل دخول للحسابات</p>
    </div>
    {#if modelStatus}
      <span class="pill">
        <span class="w-2 h-2 rounded-full {modelStatus.installed ? 'bg-emerald-400' : 'bg-amber-400'}"></span>
        {modelStatus.installed ? "النموذج جاهز" : "النموذج غير مُحمَّل"}
      </span>
    {/if}
  </header>

  {#if modelStatus && !modelStatus.installed}
    <div class="card mb-6">
      <h2 class="font-semibold mb-2">تحميل النموذج (مرة واحدة فقط)</h2>
      <p class="text-sm text-zinc-400 mb-4">
        AceGPT-v2-8B (~5.8 GB) — يُحفظ محلياً ويُستخدم للأبد بدون إنترنت.
      </p>
      {#if downloading}
        <div class="w-full bg-zinc-800 rounded-full h-3 overflow-hidden mb-2">
          <div class="bg-emerald-500 h-full transition-all" style="width: {downloadPct}%"></div>
        </div>
        <p class="text-xs text-zinc-400">
          {downloadPct}% — {downloadedMb} / {totalMb} ميجا
        </p>
      {:else}
        <button class="btn-primary" onclick={startDownload}>ابدأ التحميل</button>
      {/if}
    </div>
  {/if}

  <div class="card mb-6">
    <h2 class="font-semibold mb-4">حساب العميل</h2>
    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <div class="md:col-span-2">
        <label class="field-label" for="cn">اسم العميل</label>
        <input id="cn" class="field-input" bind:value={name} placeholder="مثلاً: فاطمة لينس" />
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
          {#each NICHES as n}
            <option value={n}>{n}</option>
          {/each}
        </select>
      </div>
      <div class="md:col-span-2">
        <span class="field-label">الدول المستهدفة</span>
        <div class="flex flex-wrap gap-2">
          {#each COUNTRIES as c}
            <button
              type="button"
              onclick={() => toggleCountry(c.code)}
              class="px-3 py-1.5 rounded-lg text-sm border transition
                {countries.includes(c.code)
                  ? 'bg-emerald-500/20 border-emerald-500 text-emerald-300'
                  : 'bg-zinc-800/70 border-zinc-700 text-zinc-300 hover:border-zinc-500'}"
            >{c.label}</button>
          {/each}
        </div>
      </div>
      <div>
        <label class="field-label" for="pd">مدة الخطة (يوم)</label>
        <select id="pd" class="field-input" bind:value={planDays}>
          {#each [7, 14, 30, 60, 90] as d}
            <option value={d}>{d} يوم</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="mt-6 flex items-center gap-3">
      <button class="btn-primary" disabled={working} onclick={generate}>
        {working ? "جارٍ التوليد..." : "توليد التقرير"}
      </button>
      <button class="btn-ghost" onclick={refreshHistory}>تحديث القائمة</button>
      {#if errorMsg}
        <span class="text-sm text-red-400">{errorMsg}</span>
      {/if}
    </div>
  </div>

  {#if lastReport}
    <div class="card mb-6">
      <h2 class="font-semibold mb-2">آخر تقرير</h2>
      <p class="text-sm text-zinc-400">
        #{lastReport.id} — {lastReport.client_name} — {new Date(lastReport.generated_at).toLocaleString("ar")}
      </p>
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
            <span>{h.client_name}</span>
            <span class="text-xs text-zinc-500">{new Date(h.generated_at).toLocaleDateString("ar")}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>
