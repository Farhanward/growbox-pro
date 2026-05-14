<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  import { marked } from "marked";
  import Icon from "$lib/Icon.svelte";

  type Status = { model_installed: boolean; server_running: boolean; model_path: string };
  type SystemPreflight = {
    ram_gb: number;
    vram_gb?: number | null;
    avx2: boolean;
    cuda: boolean;
    metal: boolean;
    recommended_profile: string;
    vision_supported: boolean;
    warnings: string[];
  };
  type LicenseStatus = {
    active: boolean;
    message: string;
    expires_at: string | null;
    days_remaining: number | null;
    renewal_warning: boolean;
    renewal_phone: string;
    features: string[];
  };
  type ConnectedAccount = {
    platform: string;
    connected: boolean;
    display_name?: string | null;
    scopes: string[];
  };
  type Report = { id: number; client_name: string; generated_at: string; status: string; content: string };
  type ScoredHashtag = { tag: string; velocity: number; relevance: number; reason: string };
  type PlatformPreview = {
    platform: string;
    caption: string;
    hashtags: string[];
    suggested_time: string;
    visual_note: string;
  };
  type GeoProfile = {
    countries: string[];
    primary_country: string;
    timezone: string;
    locale: string;
    ip_alignment_note: string;
  };
  type PublishingAssistantOutput = {
    summary: string;
    success_score: number;
    suggested_time: string;
    timezone: string;
    geo_profile: GeoProfile;
    hashtags: ScoredHashtag[];
    instagram: PlatformPreview;
    tiktok: PlatformPreview;
    virality_indicators: ViralityIndicators;
    cards: PublishingCard[];
    rationale: string[];
    data_points: string[];
    guardrails: string[];
  };
  type PublishingCard = {
    platform_info: { name: string; post_id: string };
    niche_context: { account_type: string; region: string };
    content_payload: {
      caption: string;
      hashtags: { velocity: string[]; relevance: string[] };
    };
    strategy_insights: {
      success_score: number;
      posting_time: string;
      reasoning: string;
    };
    virality_indicators: ViralityIndicators;
  };
  type ViralityIndicators = {
    hook_strength: string;
    shareability_factor: string;
    predicted_trend_alignment: string;
    strategic_score: number;
  };
  type BackgroundStatus = {
    active: boolean;
    last_run_at: string | null;
    last_summary: string;
    next_check_minutes: number;
  };
  type EvolutionStatus = {
    active: boolean;
    last_run_at: string | null;
    last_summary: string;
    next_check_days: number;
  };
  type StrategicHealthReport = { id: number; generated_at: string; content: string };
  type LoginCredentialSummary = {
    id: string;
    platform: "instagram" | "tiktok";
    username: string;
    profile_label: string;
    created_at: string;
    updated_at: string;
  };
  type VisionStatus = {
    ready: boolean;
    model_path: string;
    mmproj_path: string;
    model_hint: string;
    model_size?: number | null;
    mmproj_size?: number | null;
  };
  type VisionProgress = {
    stage: string;
    file: string;
    percent: number;
    downloaded?: number;
    total?: number;
    message: string;
  };
  type VisionInspection = {
    media_kind: string;
    description: string;
    source_note: string;
    warning?: string | null;
  };
  type IsolatedBrowserStatus = {
    running: boolean;
    port: number;
    browser?: string | null;
    profile_dir: string;
    profile_label: string;
  };
  type OAuthCallback = { platform: "instagram" | "tiktok"; code?: string | null; error?: string | null };
  type NormalizedInsights = {
    reach?: number | null;
    impressions?: number | null;
    views?: number | null;
    likes?: number | null;
    comments?: number | null;
    shares?: number | null;
    saves?: number | null;
    profile_visits?: number | null;
    engagement_rate?: number | null;
  };
  type CapturedInsights = {
    platform: string;
    post_url: string;
    captured_at: string;
    raw_endpoints: { url: string }[];
    normalized: NormalizedInsights;
    discovered_urls?: string[];
  };
  type OperationDownload = {
    label: string;
    downloaded: number;
    total?: number | null;
    startedAt: number;
  };

  const COUNTRIES = [
    { code: "SA", label: "السعودية", flag: "🇸🇦" },
    { code: "AE", label: "الإمارات", flag: "🇦🇪" },
    { code: "KW", label: "الكويت", flag: "🇰🇼" },
    { code: "QA", label: "قطر", flag: "🇶🇦" },
    { code: "BH", label: "البحرين", flag: "🇧🇭" },
    { code: "OM", label: "عُمان", flag: "🇴🇲" },
  ];
  const NICHES = ["مطعم", "مصور", "متجر ملابس", "تصوير", "موضة", "طعام", "سفر", "تجميل", "رياضة", "تعليم", "ألعاب"];

  let status = $state<Status | null>(null);
  let preflight = $state<SystemPreflight | null>(null);
  let license = $state<LicenseStatus | null>(null);
  let accounts = $state<ConnectedAccount[]>([]);
  let history = $state<Report[]>([]);

  let licenseCode = $state("");
  let licenseError = $state("");
  let setupRunning = $state(false);
  let setupError = $state("");
  let stage = $state<"idle" | "model" | "server" | "ready" | "error">("idle");
  let percent = $state(0);
  let downloadedMb = $state(0);
  let totalMb = $state(0);
  let progressMsg = $state("");
  let reportStage = $state("");
  let timerNow = $state(Date.now());
  let operationName = $state("");
  let operationPhase = $state("");
  let operationStartedAt = $state<number | null>(null);
  let operationPhaseStartedAt = $state<number | null>(null);
  let operationDownload = $state<OperationDownload | null>(null);

  let selectedPlatform = $state<"instagram" | "tiktok">("instagram");
  let oauthCode = $state("");
  let oauthMessage = $state("");
  let oauthError = $state("");

  let name = $state("");
  let tiktok = $state("");
  let instagram = $state("");
  let niche = $state(NICHES[0]);
  let countries = $state<string[]>(["SA", "KW"]);
  let mediaPath = $state("");
  let mediaNote = $state("");
  let working = $state(false);
  let formError = $state("");
  let viewingId = $state<number | null>(null);
  let viewingContent = $state("");
  let publishText = $state("");
  let copyMessage = $state("");
  let assistantWorking = $state(false);
  let assistantError = $state("");
  let assistantResult = $state<PublishingAssistantOutput | null>(null);
  let background = $state<BackgroundStatus | null>(null);
  let backgroundMsg = $state("");
  let backgroundError = $state("");
  let evolution = $state<EvolutionStatus | null>(null);
  let evolutionReports = $state<StrategicHealthReport[]>([]);
  let evolutionWorking = $state(false);
  let evolutionError = $state("");
  let evolutionRendered = $state("");
  let savedLogins = $state<LoginCredentialSummary[]>([]);
  let vaultPlatform = $state<"instagram" | "tiktok">("instagram");
  let vaultUsername = $state("");
  let vaultPassword = $state("");
  let vaultMessage = $state("");
  let vaultError = $state("");
  let vision = $state<VisionStatus | null>(null);
  let visionDownloading = $state(false);
  let visionProgress = $state<VisionProgress | null>(null);
  let visionError = $state("");
  let visionInspecting = $state(false);
  let visionInspectError = $state("");
  let visionDraft = $state("");
  let visionSourceNote = $state("");
  let visionApproved = $state(false);
  let browserStatus = $state<IsolatedBrowserStatus | null>(null);
  let browserMessage = $state("");
  let competitorPostUrls = $state("");
  let competitorInsights = $state<CapturedInsights[]>([]);
  let evidenceWorking = $state(false);
  let newAccountChecked = $state(false);
  let newAccountApproved = $state(false);

  const CUSTOMER_GUIDE = [
    "1. اطلب كود الاشتراك من الشركة ثم أدخله في شاشة التفعيل. الكود يعمل لمدة الاشتراك المحددة، وتظهر رسالة تجديد في آخر 4 أيام.",
    "2. اضغط ابدأ التجهيز لتحميل النموذج المحلي أول مرة. بعد التحميل يعمل تجهيز البوستات من داخل جهازك.",
    "3. أدخل اسم العميل وحساباته والمجال والدول المستهدفة، ثم اختر صورة أو فيديو عند الحاجة.",
    "4. اكتب وصف الوسائط والهدف من المنشور. النموذج يعتمد على وصفك والبيانات العامة، ولا يقرأ الفيديو بصرياً.",
    "5. شغّل مساعد النشر الذكي لعرض بطاقات Instagram وTikTok: الكابشن الخليجي، الهاشتاقات، وقت النشر، ودرجة النمو.",
    "6. عند الضغط على اعتماد وتجهيز المسودة، يحفظ التطبيق المسودة وينسخ الكابشن والهاشتاقات فقط بدون فتح صفحة جديدة تلقائياً.",
    "7. المستخدم يفتح صفحة النشر يدوياً عند الحاجة، يختار الوسائط داخل المنصة ويلصق النص المنسوخ ثم يضغط نشر بنفسه. التطبيق لا يضغط زر النشر نيابة عنه.",
    "8. إذا احتجت تسجيل الدخول، سجّل داخل المتصفح الرسمي فقط. الجلسة تبقى في بروفايل معزول، والتطبيق لا يقرأ كلمة السر أو الكوكيز.",
  ];

  const OWNER_LINKS = [
    { label: "Instagram", value: "kwsa.6", icon: "instagram", url: "https://www.instagram.com/kwsa.6/" },
    { label: "WhatsApp", value: "+966504211844", icon: "whatsapp", url: "https://wa.me/966504211844" },
    { label: "Email", value: "far7an.o88@gmail.com", icon: "email", url: "mailto:far7an.o88@gmail.com" },
  ];

  const IMPORTANT_LINKS = [
    { label: "TikTok Developers", url: "https://developers.tiktok.com/doc/" },
    { label: "TikTok Login Kit", url: "https://developers.tiktok.com/doc/login-kit-web" },
    { label: "TikTok Display API", url: "https://developers.tiktok.com/doc/display-api-overview" },
    { label: "Instagram API with Instagram Login", url: "https://developers.facebook.com/docs/instagram-platform/instagram-api-with-instagram-login/" },
    { label: "Meta App Dashboard", url: "https://developers.facebook.com/apps/" },
    { label: "Tauri Desktop Apps", url: "https://v2.tauri.app/" },
  ];

  onMount(() => {
    const timer = window.setInterval(() => {
      timerNow = Date.now();
    }, 1000);
    const init = async () => {
      await refreshAll();
    await listen<{ stage: string; percent: number; downloaded?: number; total?: number }>(
      "setup:progress",
      (e) => {
        stage = e.payload.stage as any;
        percent = Math.round(e.payload.percent ?? 0);
        if (e.payload.downloaded != null) downloadedMb = Math.round(e.payload.downloaded / 1_048_576);
        if (e.payload.total != null) totalMb = Math.round(e.payload.total / 1_048_576);
        setOperationPhase(setupProgressLabel(e.payload.stage), {
          label: "Hermes",
          downloaded: e.payload.downloaded ?? 0,
          total: e.payload.total,
        });
      },
    );
    await listen<{ stage: string; message: string }>("report:progress", (e) => {
      reportStage = e.payload.stage;
      progressMsg = e.payload.message;
      setOperationPhase(e.payload.message);
    });
    await listen<{ stage: string; message: string }>("fetch:progress", (e) => {
      fetchProgress = e.payload.message;
      setOperationPhase(e.payload.message);
    });
    await listen<{ stage: string; message: string }>("background:progress", (e) => {
      backgroundMsg = e.payload.message;
    });
    await listen<BackgroundStatus>("background:status", (e) => {
      background = e.payload;
      backgroundMsg = e.payload.last_summary;
    });
    await listen<EvolutionStatus>("evolution:status", (e) => {
      evolution = e.payload;
    });
    await listen<StrategicHealthReport>("evolution:report", async (e) => {
      evolutionReports = [e.payload, ...evolutionReports];
      evolutionRendered = String(await marked.parse(e.payload.content, { breaks: true }));
    });
    await listen<VisionProgress>("vision:progress", (e) => {
      visionProgress = {
        ...e.payload,
        percent: Math.round(e.payload.percent ?? 0),
      };
      setOperationPhase(e.payload.message || `Vision: ${e.payload.stage}`, e.payload.downloaded != null ? {
        label: e.payload.file || "Vision",
        downloaded: e.payload.downloaded,
        total: e.payload.total,
      } : undefined);
    });
    await listen<OAuthCallback>("oauth:callback", async (e) => {
      selectedPlatform = e.payload.platform;
      if (e.payload.error) {
        oauthError = e.payload.error;
        return;
      }
      if (e.payload.code) {
        oauthCode = e.payload.code;
        oauthMessage = "وصل كود الربط تلقائياً من المتصفح. جارٍ حفظ الربط…";
        await completeOAuth();
      }
    });
    };
    void init();
    return () => window.clearInterval(timer);
  });

  async function refreshAll() {
    await refreshLicense();
    await refreshPreflight();
    await refreshStatus();
    await refreshAccounts();
    await refreshHistory();
    await refreshBackground();
    await refreshEvolution();
    await refreshVault();
    await refreshVision();
    await refreshBrowserStatus();
  }

  async function refreshLicense() {
    try { license = await invoke<LicenseStatus>("license_status"); }
    catch (e) { licenseError = String(e); }
  }
  async function refreshStatus() { try { status = await invoke<Status>("app_status"); } catch {} }
  async function refreshPreflight() { try { preflight = await invoke<SystemPreflight>("system_preflight"); } catch {} }
  async function refreshAccounts() {
    try { if (license?.active) accounts = await invoke<ConnectedAccount[]>("get_connected_accounts"); }
    catch { accounts = []; }
  }
  async function refreshHistory() { try { history = await invoke<Report[]>("list_clients"); } catch {} }
  async function refreshBackground() { try { background = await invoke<BackgroundStatus>("background_status"); } catch {} }
  async function refreshEvolution() {
    try {
      evolution = await invoke<EvolutionStatus>("evolution_status");
      if (license?.active) evolutionReports = await invoke<StrategicHealthReport[]>("list_evolution_reports");
      if (evolutionReports[0]) evolutionRendered = String(await marked.parse(evolutionReports[0].content, { breaks: true }));
    } catch {}
  }
  async function refreshVault() {
    try { if (license?.active) savedLogins = await invoke<LoginCredentialSummary[]>("list_login_credentials"); }
    catch { savedLogins = []; }
  }
  async function refreshVision() { try { vision = await invoke<VisionStatus>("vision_status"); } catch {} }
  async function refreshBrowserStatus() { try { browserStatus = await invoke<IsolatedBrowserStatus>("isolated_browser_status"); } catch {} }

  function startOperation(name: string, phase: string) {
    const now = Date.now();
    operationName = name;
    operationPhase = phase;
    operationStartedAt = now;
    operationPhaseStartedAt = now;
    operationDownload = null;
    timerNow = now;
  }

  function setOperationPhase(phase: string, download?: { label: string; downloaded?: number; total?: number | null }) {
    const now = Date.now();
    if (!operationStartedAt) {
      startOperation("عملية جارية", phase);
    } else if (phase && phase !== operationPhase) {
      operationPhase = phase;
      operationPhaseStartedAt = now;
    }
    if (download && download.downloaded != null) {
      operationDownload = {
        label: download.label,
        downloaded: download.downloaded,
        total: download.total,
        startedAt: operationDownload?.label === download.label ? operationDownload.startedAt : now,
      };
    } else if (!download && !/تحميل|download/i.test(phase)) {
      operationDownload = null;
    }
    timerNow = now;
  }

  function finishOperation() {
    operationName = "";
    operationPhase = "";
    operationStartedAt = null;
    operationPhaseStartedAt = null;
    operationDownload = null;
  }

  function setupProgressLabel(value: string) {
    if (value === "model") return "تحميل نموذج Hermes";
    if (value === "server") return "تشغيل محرك Hermes";
    if (value === "ready") return "اكتمل تجهيز Hermes";
    return "تجهيز Hermes";
  }

  async function activate() {
    licenseError = "";
    try {
      license = await invoke<LicenseStatus>("activate_license", { code: licenseCode });
      licenseCode = "";
      await refreshAll();
    } catch (e) { licenseError = String(e); }
  }

  async function runSetup() {
    setupRunning = true; setupError = ""; stage = "model"; percent = 0;
    startOperation("تجهيز Hermes", "تحميل نموذج Hermes");
    try {
      await invoke("setup_app");
      await refreshStatus();
      stage = "ready";
    } catch (e) { setupError = String(e); stage = "error"; }
    finally { setupRunning = false; finishOperation(); }
  }

  async function downloadVisionModel() {
    visionDownloading = true;
    visionError = "";
    visionProgress = { stage: "model", file: "Qwen2-VL", percent: 0, message: "بدء تحميل النموذج البصري..." };
    startOperation("تحميل النموذج البصري", "بدء تحميل Qwen2-VL");
    try {
      vision = await invoke<VisionStatus>("download_vision_model");
      await refreshVision();
    } catch (e) {
      visionError = String(e);
    } finally {
      visionDownloading = false;
      finishOperation();
    }
  }

  async function beginOAuth(platform: "instagram" | "tiktok") {
    oauthError = ""; oauthMessage = "";
    try {
      const url = await invoke<string>("start_oauth", { platform });
      await openUrl(url);
      oauthMessage = "تم فتح صفحة الربط في المتصفح. بعد الموافقة سيعود الكود للتطبيق تلقائياً؛ وإن لم يحدث، الصقه هنا يدوياً.";
    } catch (e) { oauthError = String(e); }
  }

  async function openOfficialView(platform: "instagram" | "tiktok") {
    oauthError = ""; oauthMessage = "";
    try {
      const accountLabel = isolatedProfileLabel(platform);
      browserStatus = await invoke<IsolatedBrowserStatus>("launch_isolated_browser", { platform, accountLabel });
      browserMessage = `تم تشغيل ${browserStatus.browser ?? "المتصفح الرسمي"} ببروفايل ${browserStatus.profile_label} ومنفذ ${browserStatus.port}.`;
      oauthMessage = "الجلسة تبقى داخل بروفايل المتصفح المعزول فقط. التطبيق لا يقرأ كلمات السر أو الكوكيز.";
    } catch (e) { oauthError = String(e); }
  }

  function isolatedProfileLabel(platform: "instagram" | "tiktok") {
    const handle = platform === "instagram" ? instagram : tiktok;
    return handle.trim() || name.trim() || "default";
  }

  function strategicScoreStyle(score: number) {
    if (score > 80) return "border-color: rgba(34,197,94,.45); color: #86efac; background: rgba(34,197,94,.10)";
    if (score >= 50) return "border-color: rgba(245,158,11,.48); color: #fcd34d; background: rgba(245,158,11,.10)";
    return "border-color: rgba(239,68,68,.48); color: #fca5a5; background: rgba(239,68,68,.10)";
  }

  function draftProgressPercent(stageName: string) {
    const map: Record<string, number> = {
      draft_start: 8,
      gather: 22,
      account_signals: 30,
      trends: 42,
      draft_context: 55,
      draft_llm: 76,
      draft_save: 92,
      draft_ready: 100,
    };
    return map[stageName] ?? (working ? 48 : 0);
  }

  function draftProgressLabel(stageName: string) {
    if (stageName === "draft_llm") return "صياغة البوست";
    if (stageName === "draft_save") return "حفظ النتيجة";
    if (stageName === "draft_ready") return "جاهز";
    if (stageName === "draft_context") return "تنظيم البيانات";
    if (stageName === "draft_start") return "بدء الاعتماد";
    return "جمع الإشارات";
  }

  async function stopIsolatedBrowser() {
    try {
      browserStatus = await invoke<IsolatedBrowserStatus>("stop_isolated_browser");
      browserMessage = "تم إغلاق المتصفح المعزول.";
    } catch (e) { oauthError = String(e); }
  }

  async function saveVaultLogin() {
    vaultError = ""; vaultMessage = "";
    try {
      await invoke<LoginCredentialSummary>("save_login_credential", {
        input: { platform: vaultPlatform, username: vaultUsername.trim(), password: vaultPassword },
      });
      vaultPassword = "";
      vaultMessage = "تم حفظ بيانات الدخول محلياً بتشفير AES-256.";
      await refreshVault();
    } catch (e) { vaultError = String(e); }
  }

  async function deleteVaultLogin(id: string) {
    vaultError = ""; vaultMessage = "";
    try {
      await invoke("delete_login_credential", { id });
      vaultMessage = "تم حذف بيانات الدخول.";
      await refreshVault();
    } catch (e) { vaultError = String(e); }
  }

  async function openSavedSession(id: string) {
    vaultError = ""; vaultMessage = "";
    try {
      browserStatus = await invoke<IsolatedBrowserStatus>("launch_saved_account_session", { id });
      browserMessage = `تم فتح جلسة محفوظة ببروفايل ${browserStatus.profile_label}.`;
    } catch (e) { vaultError = String(e); }
  }

  async function toggleEvolutionTracker() {
    evolutionError = "";
    try {
      evolution = evolution?.active
        ? await invoke<EvolutionStatus>("stop_evolution_tracker")
        : await invoke<EvolutionStatus>("start_evolution_tracker");
    } catch (e) { evolutionError = String(e); }
  }

  async function runEvolutionNow() {
    evolutionError = ""; evolutionWorking = true;
    try {
      const report = await invoke<StrategicHealthReport>("run_evolution_report");
      evolutionReports = [report, ...evolutionReports];
      evolutionRendered = String(await marked.parse(report.content, { breaks: true }));
      await refreshEvolution();
    } catch (e) { evolutionError = String(e); }
    finally { evolutionWorking = false; }
  }

  async function completeOAuth() {
    oauthError = ""; oauthMessage = "";
    try {
      await invoke("complete_oauth", { platform: selectedPlatform, code: oauthCode.trim() });
      oauthCode = "";
      oauthMessage = "تم حفظ الربط بأمان في مخزن النظام.";
      await refreshAccounts();
    } catch (e) { oauthError = String(e); }
  }

  async function chooseMedia() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Media", extensions: ["png", "jpg", "jpeg", "webp", "mp4", "mov"] }],
    });
    if (typeof selected === "string") {
      mediaPath = selected;
      assistantResult = null;
      visionDraft = "";
      visionSourceNote = "";
      visionApproved = false;
      mediaNote = "";
    }
  }

  async function inspectMedia() {
    visionInspectError = "";
    if (!mediaPath.trim()) { visionInspectError = "اختر صورة أو فيديو أولاً."; return; }
    if (!vision?.ready) { visionInspectError = "حمّل Qwen2-VL من خطوة التجهيز أولاً."; return; }
    visionInspecting = true;
    visionApproved = false;
    assistantResult = null;
    startOperation("قراءة الوسائط", "تجهيز Qwen2-VL");
    try {
      const result = await invoke<VisionInspection>("inspect_media_with_vision", { mediaPath });
      visionDraft = result.description.trim();
      visionSourceNote = result.source_note;
      if (result.warning) visionInspectError = result.warning;
    } catch (e) {
      visionInspectError = String(e);
    } finally {
      visionInspecting = false;
      finishOperation();
    }
  }

  function approveVisionDescription() {
    if (!visionDraft.trim()) {
      visionInspectError = "لا يوجد وصف بصري للموافقة عليه.";
      return;
    }
    mediaNote = visionDraft.trim();
    visionApproved = true;
    assistantResult = null;
    visionInspectError = "";
  }

  function toggleCountry(code: string) {
    countries = countries.includes(code) ? countries.filter((c) => c !== code) : [...countries, code];
    assistantResult = null;
  }

  function draftInput() {
    return {
      platform: selectedPlatform,
      media_path: mediaPath || null,
      media_note: mediaNote,
      client: {
        name: name.trim(),
        tiktok: tiktok.trim() || null,
        instagram: instagram.trim() || null,
        snapchat: null,
        niche,
        countries,
        plan_days: 30,
      },
    };
  }

  function isAccountProfileUrl(url: string) {
    const value = url.trim().toLowerCase();
    if (!value) return false;
    if (selectedPlatform === "tiktok") {
      return value.includes("tiktok.com/@") && !value.includes("/video/");
    }
    if (value.includes("instagram.com")) {
      return !value.includes("/p/") && !value.includes("/reel/") && !value.includes("/tv/");
    }
    return false;
  }

  function hasRealMetrics(insights: CapturedInsights | null) {
    if (!insights) return false;
    const n = insights.normalized;
    return [
      n.reach,
      n.impressions,
      n.views,
      n.likes,
      n.comments,
      n.shares,
      n.saves,
      n.profile_visits,
      n.engagement_rate,
    ].some((value) => value != null);
  }

  function emptyOwnPostBaseline(): CapturedInsights {
    return {
      platform: selectedPlatform,
      post_url: "حساب جديد بلا منشورات - بدء من الصفر",
      captured_at: new Date().toISOString(),
      raw_endpoints: [],
      normalized: {},
      discovered_urls: [],
    };
  }

  function toggleNewAccountMode(checked: boolean) {
    newAccountChecked = checked;
    newAccountApproved = false;
    if (checked) {
      fetchPostUrl = "";
      fetchedInsights = null;
      fetchError = "";
      fetchProgress = "";
    }
  }

  function approveNewAccountMode() {
    if (!newAccountChecked) return;
    newAccountApproved = true;
    fetchPostUrl = "";
    fetchedInsights = emptyOwnPostBaseline();
    fetchError = "";
    fetchProgress = "تم اعتماد الحساب كحساب جديد بلا منشورات. سيعمل التحليل على تسريع انتشار أول منشور اعتماداً على Vision + المنافسين.";
  }

  async function fetchInsights() {
    if (newAccountApproved) {
      fetchError = "تم اعتماد الحساب كبداية من الصفر. ألغِ الخيار إذا أردت جلب أرقام منشور منشور فعلاً.";
      return;
    }
    if (!fetchPostUrl.trim()) { fetchError = "أدخل رابط المنشور."; return; }
    if (isAccountProfileUrl(fetchPostUrl)) {
      fetchError = "هذا رابط حساب وليس رابط منشور. بما أن الحساب تجريبي بلا منشورات، اترك هذه الخانة فارغة وشغّل التحليل ليعتمد على Vision + المنافسين فقط.";
      fetchedInsights = null;
      return;
    }
    fetchWorking = true; fetchError = ""; fetchProgress = ""; fetchedInsights = null;
    startOperation("جلب بيانات الحساب", "الاتصال بالمتصفح المعزول");
    try {
      fetchedInsights = await invoke<CapturedInsights>("fetch_post_insights", {
        platform: selectedPlatform,
        postUrl: fetchPostUrl.trim(),
      });
      if (hasRealMetrics(fetchedInsights)) {
        fetchProgress = "تم جلب بيانات المنشور بنجاح.";
      } else {
        fetchError = "تم فتح الرابط، لكن لم تظهر أرقام منشور فعلية. استخدم رابط منشور مباشر، أو اترك الخانة فارغة للحساب التجريبي بلا منشورات.";
        fetchedInsights = null;
      }
    } catch (e) { fetchError = String(e); }
    finally { fetchWorking = false; finishOperation(); }
  }

  async function runAnalysisWithFetched() {
    if (!license?.active) { assistantError = "فعّل الاشتراك أولاً."; return; }
    if (!name.trim()) { assistantError = "أدخل اسم العميل."; return; }
    if (!mediaNote.trim()) { assistantError = "اكتب وصف الصورة أو الفيديو."; return; }
    if (countries.length === 0) { assistantError = "اختر دولة واحدة على الأقل."; return; }
    if (!fetchedInsights) { assistantError = "اجلب إحصائيات المنشور أولاً."; return; }
    assistantWorking = true; assistantError = ""; assistantResult = null; progressMsg = "تحليل مُعزَّز بالبيانات الفعلية…";
    startOperation("تحليل البيانات الفعلية", "تشغيل Hermes");
    try {
      assistantResult = await invoke<PublishingAssistantOutput>("analyze_with_fetched_insights", {
        input: draftInput(),
        fetched: fetchedInsights,
      });
    } catch (e) { assistantError = String(e); }
    finally { assistantWorking = false; progressMsg = ""; finishOperation(); }
  }

  function competitorUrls() {
    return competitorPostUrls
      .split(/\r?\n|,/)
      .map((url) => url.trim())
      .filter(Boolean)
      .filter((url, index, all) => all.indexOf(url) === index)
      .slice(0, 12);
  }

  function seedHashtagsForNiche() {
    const map: Record<string, string[]> = {
      "مصور": ["#تصوير_احترافي", "#مصورين_السعودية", "#تصوير_منتجات", "#فوتوغرافي"],
      "تصوير": ["#تصوير_احترافي", "#مصورين_السعودية", "#تصوير_منتجات", "#فوتوغرافي"],
      "مطعم": ["#مطاعم_الرياض", "#foodie_sa", "#كافيهات", "#مطاعم_جده"],
      "طعام": ["#مطاعم_الرياض", "#foodie_sa", "#كافيهات", "#مطاعم_جده"],
      "موضة": ["#موضة_خليجية", "#ستايل", "#عبايات", "#لوك_اليوم"],
      "متجر ملابس": ["#موضة_خليجية", "#ستايل", "#عبايات", "#لوك_اليوم"],
      "تجميل": ["#مكياج", "#عناية_بشرة", "#beauty_arabia", "#تجميل_خليجي"],
      "رياضة": ["#لياقة", "#جيم", "#تمارين", "#fitness_sa"],
      "تعليم": ["#تعليم", "#دورات", "#مهارات", "#study_arabia"],
      "ألعاب": ["#قيمنق", "#gaming_arabia", "#esports_ksa", "#قيمر_عربي"],
    };
    return map[niche] ?? [`#${niche}`];
  }

  async function fetchOneInsight(postUrl: string) {
    return await invoke<CapturedInsights>("fetch_post_insights", {
      platform: selectedPlatform,
      postUrl,
    });
  }

  async function runEvidenceAnalysis() {
    if (!license?.active) { assistantError = "فعّل الاشتراك أولاً."; return; }
    if (!name.trim()) { assistantError = "أدخل اسم العميل."; return; }
    if (!mediaNote.trim() || !visionApproved) { assistantError = "اقرأ الوسائط بالنموذج البصري ثم وافق على الوصف أولاً."; return; }
    if (!fetchPostUrl.trim() && !newAccountApproved) {
      assistantError = "إما ضع رابط منشور منشور فعلاً، أو فعّل خيار: الحساب جديد بلا منشورات ثم اضغط موافق.";
      return;
    }
    if (fetchPostUrl.trim() && isAccountProfileUrl(fetchPostUrl)) {
      assistantError = "هذا رابط حساب وليس رابط منشور. للحساب التجريبي بلا منشورات، امسح الخانة وشغّل التحليل بالمنافسين فقط.";
      return;
    }
    if (countries.length === 0) { assistantError = "اختر دولة واحدة على الأقل."; return; }

    evidenceWorking = true;
    assistantWorking = true;
    assistantError = "";
    fetchError = "";
    assistantResult = null;
    progressMsg = "جلب إحصاءات منشور الحساب...";
    startOperation("تحليل بالأدلة الفعلية", "جلب إحصاءات منشور الحساب");
    try {
      let primary = fetchedInsights;
      if (fetchPostUrl.trim() && !primary) {
        primary = await fetchOneInsight(fetchPostUrl.trim());
        if (!hasRealMetrics(primary)) {
          fetchError = "لم يتم العثور على أرقام منشور فعلية في رابط حسابك؛ سيتم اعتبار الحساب بلا baseline حالياً.";
          primary = emptyOwnPostBaseline();
        }
      }
      if (!primary) {
        primary = emptyOwnPostBaseline();
        fetchProgress = "الحساب جديد بلا منشورات؛ سيتم بناء خطة أول منشور من Vision + المنافسين فقط.";
      }
      fetchedInsights = primary;
      let urls = competitorUrls();
      if (urls.length === 0) {
        progressMsg = "لا توجد روابط منافسين؛ بدء الاستكشاف التلقائي بالهاشتاقات...";
        setOperationPhase(progressMsg);
        try {
          urls = await invoke<string[]>("discover_competitor_posts", {
            platform: selectedPlatform,
            hashtags: seedHashtagsForNiche(),
          });
          competitorPostUrls = urls.join("\n");
        } catch (e) {
          fetchError = `تعذر الاستكشاف التلقائي: ${String(e)}`;
        }
      }
      const competitors: CapturedInsights[] = [];
      for (const [index, url] of urls.entries()) {
        progressMsg = `جلب منشور منافس ${index + 1} من ${urls.length}...`;
        setOperationPhase(progressMsg);
        try {
          competitors.push(await fetchOneInsight(url));
        } catch (e) {
          fetchError = `تعذر جلب بعض المنافسين: ${String(e)}`;
        }
      }
      competitorInsights = competitors;
      progressMsg = "تمرير وصف Vision والإحصاءات الفعلية إلى Hermes...";
      setOperationPhase(progressMsg);
      assistantResult = await invoke<PublishingAssistantOutput>("analyze_with_market_evidence", {
        input: draftInput(),
        fetched: primary,
        competitors,
      });
    } catch (e) {
      assistantError = String(e);
    } finally {
      evidenceWorking = false;
      assistantWorking = false;
      progressMsg = "";
      finishOperation();
    }
  }

  async function runPublishingAssistant() {
    if (!license?.active) { assistantError = "فعّل الاشتراك أولاً."; return; }
    if (!name.trim()) { assistantError = "أدخل اسم العميل."; return; }
    if (!mediaNote.trim()) { assistantError = "اكتب وصف الصورة أو الفيديو."; return; }
    if (countries.length === 0) { assistantError = "اختر دولة واحدة على الأقل."; return; }
    assistantWorking = true; assistantError = ""; assistantResult = null; progressMsg = "تشغيل مساعد النشر الذكي…";
    startOperation("مساعد النشر الذكي", "تشغيل Hermes");
    try {
      assistantResult = await invoke<PublishingAssistantOutput>("analyze_publish_strategy", { input: draftInput() });
    } catch (e) { assistantError = String(e); }
    finally { assistantWorking = false; progressMsg = ""; finishOperation(); }
  }

  async function toggleBackgroundMode() {
    backgroundError = "";
    try {
      if (background?.active) {
        background = await invoke<BackgroundStatus>("stop_background_mode");
        backgroundMsg = background.last_summary;
        return;
      }
      if (!name.trim()) { backgroundError = "أدخل اسم العميل قبل تشغيل وضع الخلفية."; return; }
      background = await invoke<BackgroundStatus>("start_background_mode", { input: draftInput().client });
      backgroundMsg = background.last_summary;
    } catch (e) {
      backgroundError = String(e);
    }
  }

  async function sendToTray() {
    try { await invoke("minimize_to_tray"); }
    catch (e) { backgroundError = String(e); }
  }

  async function generateDraft() {
    if (!license?.active) { formError = "فعّل الاشتراك أولاً."; return; }
    if (!name.trim()) { formError = "أدخل اسم العميل."; return; }
    if (!mediaNote.trim()) { formError = "اكتب وصف الصورة أو الفيديو."; return; }
    if (!assistantResult) { formError = "شغّل مساعد النشر الذكي واعتمد المعاينة قبل تجهيز المسودة."; return false; }
    working = true; formError = ""; reportStage = "draft_start"; progressMsg = "بدء اعتماد المسودة النهائية…";
    startOperation("اعتماد وتجهيز المسودة", "بدء اعتماد المسودة النهائية");
    try {
      if (!publishText.trim()) publishText = previewPublishText();
      const r = await invoke<Report>("generate_post_draft", {
        input: draftInput(),
      });
      viewingId = r.id;
      viewingContent = r.content;
      await refreshHistory();
      return true;
    } catch (e) { formError = String(e); return false; }
    finally { working = false; progressMsg = ""; reportStage = ""; finishOperation(); }
  }

  function editCard(card: PublishingCard) {
    mediaNote = card.content_payload.caption;
    assistantResult = null;
    formError = "تم نقل الكابشن إلى خانة الوصف للتعديل، ثم أعد تشغيل التحليل.";
  }

  async function approveCard(card: PublishingCard) {
    selectedPlatform = card.platform_info.name.toLowerCase().includes("tiktok") ? "tiktok" : "instagram";
    publishText = cardPublishText(card);
    const ok = await generateDraft();
    if (ok) {
      await copyDraft();
      browserMessage = "تم اعتماد المسودة ونسخ الكابشن والهاشتاقات. لن يتم فتح صفحة نشر جديدة تلقائياً؛ افتحها يدوياً عند الحاجة.";
    }
  }

  async function viewReport(id: number) {
    viewingContent = await invoke<string>("load_report", { id });
    viewingId = id;
  }

  async function copyText(text: string, success = "تم النسخ.") {
    copyMessage = "";
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const area = document.createElement("textarea");
      area.value = text;
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.appendChild(area);
      area.focus();
      area.select();
      document.execCommand("copy");
      document.body.removeChild(area);
    }
    copyMessage = success;
    window.setTimeout(() => {
      if (copyMessage === success) copyMessage = "";
    }, 2200);
  }

  async function copyDraft() {
    const text = publishText.trim() || extractPublishText(viewingContent);
    if (text) await copyText(text, "تم نسخ الكابشن والهاشتاقات.");
  }

  async function openPublishPages() {
    const text = publishText.trim() || extractPublishText(viewingContent);
    if (text) await copyText(text, "تم نسخ الكابشن والهاشتاقات. الصقه في صفحة النشر بعد اختيار الوسائط.");
    try {
      const accountLabel = isolatedProfileLabel(selectedPlatform);
      browserStatus = await invoke<IsolatedBrowserStatus>("launch_publish_page", {
        platform: selectedPlatform,
        accountLabel,
      });
      browserMessage = `تم فتح صفحة نشر ${selectedPlatform === "instagram" ? "Instagram" : "TikTok"} بالبروفايل نفسه. اختر الوسائط ثم الصق الكابشن المنسوخ واضغط نشر بنفسك.`;
    } catch (e) {
      formError = String(e);
    }
  }

  function cardPublishText(card: PublishingCard) {
    const tags = [...card.content_payload.hashtags.velocity, ...card.content_payload.hashtags.relevance]
      .filter((tag, index, all) => tag && all.indexOf(tag) === index)
      .join(" ");
    return `${card.content_payload.caption.trim()}\n\n${tags}`.trim();
  }

  function previewPublishText() {
    if (!activePreview) return "";
    return `${activePreview.caption.trim()}\n\n${activePreview.hashtags.join(" ")}`.trim();
  }

  function extractPublishText(markdown: string) {
    if (!markdown.trim()) return "";
    const caption = markdown.match(/##\s*الكابشن الجاهز\s*([\s\S]*?)(?=\n##\s|$)/)?.[1]?.trim() ?? "";
    const hashtags = markdown.match(/##\s*الهاشتاقات\s*([\s\S]*?)(?=\n##\s|$)/)?.[1]?.trim() ?? "";
    return `${caption}\n\n${hashtags}`.trim() || markdown.trim();
  }

  async function copyLicenseMessage() {
    if (!license?.message) return;
    await copyText(license.message, "تم نسخ رسالة الاشتراك.");
  }

  async function copyOAuthCode() {
    if (!oauthCode.trim()) return;
    await copyText(oauthCode.trim(), "تم نسخ كود الربط.");
  }

  function formatMb(bytes?: number | null) {
    if (!bytes) return "0 MB";
    return `${Math.round(bytes / 1_048_576).toLocaleString("ar")} MB`;
  }

  function formatDuration(ms: number) {
    const totalSeconds = Math.max(0, Math.floor(ms / 1000));
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    const two = (value: number) => value.toString().padStart(2, "0");
    return hours > 0 ? `${hours}:${two(minutes)}:${two(seconds)}` : `${two(minutes)}:${two(seconds)}`;
  }

  function formatRate(bytesPerSecond: number) {
    if (!Number.isFinite(bytesPerSecond) || bytesPerSecond <= 0) return "0 MB/s";
    return `${(bytesPerSecond / 1_048_576).toLocaleString("ar", { maximumFractionDigits: 2 })} MB/s`;
  }

  function downloadMarkdown() {
    if (!viewingContent) return;
    const blob = new Blob([viewingContent], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url; a.download = `growbox-post-${viewingId}.md`; a.click();
    URL.revokeObjectURL(url);
  }

  // Data Fetching Module state
  let fetchPostUrl = $state("");
  let fetchWorking = $state(false);
  let fetchError = $state("");
  let fetchProgress = $state("");
  let fetchedInsights = $state<CapturedInsights | null>(null);

  let isReady = $derived(Boolean(status?.model_installed));
  let rendered = $derived(viewingContent ? marked.parse(viewingContent, { breaks: true }) : "");
  let stageMsg = $derived(stage === "model" ? "تحميل النموذج" : stage === "server" ? "تشغيل المحرّك" : "جاهز");
  let connectedLabel = $derived(accounts.filter((a) => a.connected).map((a) => a.platform).join(" + ") || "لا يوجد ربط");
  let activePreview = $derived(assistantResult ? (selectedPlatform === "instagram" ? assistantResult.instagram : assistantResult.tiktok) : null);
  let visionPercent = $derived(visionProgress?.percent ?? (vision?.ready ? 100 : 0));
  let pipelineBusy = $derived(setupRunning || visionDownloading || visionInspecting || fetchWorking || evidenceWorking || assistantWorking);
  let pipelineBusyMessage = $derived(
    setupRunning ? stageMsg :
    visionDownloading ? (visionProgress?.message || "تحميل النموذج البصري") :
    visionInspecting ? "Qwen2-VL يقرأ الوسائط" :
    fetchWorking ? (fetchProgress || "جلب البيانات من الجلسة المعزولة") :
    evidenceWorking ? (progressMsg || "تحليل الأدلة الفعلية") :
    assistantWorking ? (progressMsg || "Hermes يكتب البطاقات") :
    "جاري العمل",
  );
  let operationElapsed = $derived(operationStartedAt ? formatDuration(timerNow - operationStartedAt) : "00:00");
  let operationPhaseElapsed = $derived(operationPhaseStartedAt ? formatDuration(timerNow - operationPhaseStartedAt) : "00:00");
  let operationProgress = $derived(
    working ? draftProgressPercent(reportStage) :
    visionDownloading ? visionPercent :
    setupRunning ? percent :
    visionInspecting ? visionPercent :
    null,
  );
  let operationRate = $derived(
    operationDownload && operationDownload.startedAt
      ? operationDownload.downloaded / Math.max(1, (timerNow - operationDownload.startedAt) / 1000)
      : 0,
  );
  let operationMeasuredLine = $derived(
    operationDownload
      ? `${operationDownload.label}: ${formatMb(operationDownload.downloaded)}${operationDownload.total ? ` من ${formatMb(operationDownload.total)}` : ""} · ${formatRate(operationRate)}`
      : "العداد يعتمد على الزمن الفعلي للمرحلة الجارية.",
  );
</script>

<div class="min-h-screen" dir="rtl">
  <header class="vibrancy sticky top-0 z-10 px-6 py-4">
    <div class="max-w-6xl mx-auto flex flex-wrap items-center justify-between gap-4">
      <div class="flex items-center gap-3">
        <img src="/growbox-brand.png" alt="GrowBox Pro" class="w-11 h-11 rounded-lg object-cover" />
        <div>
          <h1 class="text-[18px] font-semibold leading-tight">GrowBox Pro</h1>
          <p class="text-[12px]" style="color: var(--text-2)">مسار نشر محلي منظم، خطوة بعد خطوة</p>
        </div>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <span class="pill">
          <span class="status-dot" style="background: {license?.active ? 'var(--success)' : 'var(--danger)'}"></span>
          {license?.active ? `اشتراك فعال · ${license.days_remaining} يوم` : "غير مفعل"}
        </span>
        <span class="pill">
          <span class="status-dot" style="background: {isReady ? 'var(--success)' : 'var(--warning)'}"></span>
          Hermes {isReady ? "مثبت" : "يحتاج تجهيز"}
        </span>
        <span class="pill">
          <span class="status-dot" style="background: {vision?.ready ? 'var(--success)' : 'var(--warning)'}"></span>
          Vision {vision?.ready ? "جاهز" : "غير محمل"}
        </span>
      </div>
    </div>
  </header>

  <main class="max-w-6xl mx-auto px-6 py-8 space-y-5">
    {#if !license?.active}
      <section class="surface step-card">
        <div class="step-heading">
          <span class="step-number">0</span>
          <div>
            <h2>تفعيل الاشتراك</h2>
            <p>ابدأ من هنا فقط. بعد التفعيل تظهر خطوات التشغيل والنشر.</p>
          </div>
        </div>
        <div class="grid grid-cols-1 md:grid-cols-[1fr_auto] gap-3 mt-5">
          <input class="field-input" bind:value={licenseCode} placeholder="GBX-..." />
          <button class="btn btn-primary" onclick={activate}><Icon name="check" size={18} /> تفعيل</button>
        </div>
        {#if licenseError}<p class="mt-4 text-[13px]" style="color: var(--danger)">{licenseError}</p>{/if}
        {#if license}<p class="mt-4 text-[13px]" style="color: var(--text-2)">{license.message}</p>{/if}
      </section>
    {:else}
      {#if license.renewal_warning}
        <section class="surface p-4 border border-[var(--warning)]">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <p class="font-medium" style="color: var(--warning)">{license.message}</p>
            <button class="btn btn-ghost btn-small" onclick={copyLicenseMessage}><Icon name="check" size={14} /> نسخ الرسالة</button>
          </div>
        </section>
      {/if}

      <nav class="workflow-strip" aria-label="خطوات العمل">
        <span class="workflow-step {isReady && vision?.ready ? 'done' : ''}"><b>1</b> التجهيز</span>
        <span class="workflow-step {name.trim() ? 'done' : ''}"><b>2</b> الحساب</span>
        <span class="workflow-step {visionApproved ? 'done' : ''}"><b>3</b> قراءة الوسائط</span>
        <span class="workflow-step {assistantResult ? 'done' : ''}"><b>4</b> الأدلة الفعلية</span>
        <span class="workflow-step {viewingContent ? 'done' : ''}"><b>5</b> النشر اليدوي</span>
        <span class="workflow-step {evolution?.active || evolutionReports.length ? 'done' : ''}"><b>6</b> التطور</span>
      </nav>

      <section class="surface step-card">
        <div class="step-heading">
          <span class="step-number">1</span>
          <div>
            <h2>تجهيز المحركات المحلية</h2>
            <p>Hermes يكتب النص النهائي، وQwen2-VL يصف الصورة أو الفيديو قبل الصياغة.</p>
          </div>
        </div>
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4 mt-5">
          <div class="setup-panel">
            <div>
              <p class="field-label">Hermes-2-Pro</p>
              <h3>{isReady ? "مثبت وسيعمل عند الطلب" : "يحتاج تحميل"}</h3>
              <p>{status?.model_path || "لم يتم فحص المسار بعد."}</p>
            </div>
            <button class="btn {isReady ? 'btn-ghost' : 'btn-primary'}" disabled={setupRunning} onclick={runSetup}>
              {#if setupRunning}<Icon name="spinner" size={18} class="animate-spin" /> {stageMsg}{:else}<Icon name="play" size={18} /> {isReady ? "إعادة الفحص" : "تجهيز Hermes"}{/if}
            </button>
            {#if setupRunning}
              <div class="progress-track"><div class="progress-fill" style="width: {percent}%"></div></div>
              <p class="text-[12px]" style="color: var(--text-2)">{percent}%{stage === "model" && totalMb ? ` · ${downloadedMb} من ${totalMb} MB` : ""}</p>
            {/if}
            {#if setupError}<p class="text-[13px]" style="color: var(--danger)">{setupError}</p>{/if}
          </div>
          <div class="setup-panel">
            <div>
              <p class="field-label">Qwen2-VL Vision Agent</p>
              <h3>{vision?.ready ? "جاهز لتحليل الوسائط" : "لم يتم تحميل النموذج البصري"}</h3>
              <p>{vision?.ready ? `${formatMb(vision.model_size)} + ${formatMb(vision.mmproj_size)}` : vision?.model_hint}</p>
            </div>
            <button class="btn {vision?.ready ? 'btn-ghost' : 'btn-primary'}" disabled={visionDownloading} onclick={downloadVisionModel}>
              {#if visionDownloading}<Icon name="spinner" size={18} class="animate-spin" /> تحميل {visionProgress?.file}{:else}<Icon name="download" size={18} /> {vision?.ready ? "فحص النموذج" : "تحميل Qwen2-VL"}{/if}
            </button>
            {#if visionDownloading || visionProgress}
              <div class="progress-track"><div class="progress-fill" style="width: {visionPercent}%"></div></div>
              <p class="text-[12px]" style="color: var(--text-2)">
                {visionProgress?.message || "جاهز"} · {visionPercent}%{visionProgress?.total ? ` · ${formatMb(visionProgress.downloaded)} من ${formatMb(visionProgress.total)}` : ""}
              </p>
            {/if}
            {#if visionError}<p class="text-[13px]" style="color: var(--danger)">{visionError}</p>{/if}
          </div>
        </div>
        {#if preflight}
          <div class="metric-strip">
            <span>RAM {preflight.ram_gb}GB</span>
            <span>VRAM {preflight.vram_gb ?? "غير معروف"}GB</span>
            <span>AVX2 {preflight.avx2 ? "مدعوم" : "غير مدعوم"}</span>
            <span>CUDA {preflight.cuda ? "مدعوم" : "غير متاح"}</span>
            <span>Profile {preflight.recommended_profile}</span>
          </div>
          {#if preflight.warnings.length}
            <div class="mt-3 text-[13px]" style="color: var(--warning)">
              {#each preflight.warnings as warning}
                <p>{warning}</p>
              {/each}
            </div>
          {/if}
        {/if}
      </section>

      <section class="surface step-card">
        <div class="step-heading">
          <span class="step-number">2</span>
          <div>
            <h2>بيانات الحساب والجلسة</h2>
            <p>اختر المنصة والحساب المستهدف. الجلسات المحفوظة تستخدم بروفايل متصفح مستقل حتى لا يضيع تسجيل الدخول.</p>
          </div>
        </div>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-5">
          <div>
            <label class="field-label" for="platform">المنصة</label>
            <select id="platform" class="field-input" bind:value={selectedPlatform} onchange={() => assistantResult = null}>
              <option value="instagram">Instagram</option>
              <option value="tiktok">TikTok</option>
            </select>
          </div>
          <div>
            <label class="field-label" for="cn">اسم العميل</label>
            <input id="cn" class="field-input" bind:value={name} oninput={() => assistantResult = null} placeholder="مثال: فاطمة لينس" />
          </div>
          <div>
            <label class="field-label" for="ig">Instagram</label>
            <input id="ig" class="field-input" bind:value={instagram} oninput={() => assistantResult = null} placeholder="@username" />
          </div>
          <div>
            <label class="field-label" for="tt">TikTok</label>
            <input id="tt" class="field-input" bind:value={tiktok} oninput={() => assistantResult = null} placeholder="@username" />
          </div>
          <div>
            <label class="field-label" for="nc">المجال</label>
            <select id="nc" class="field-input" bind:value={niche} onchange={() => assistantResult = null}>
              {#each NICHES as n}<option value={n}>{n}</option>{/each}
            </select>
          </div>
          <div>
            <span class="field-label">الدول المستهدفة</span>
            <div class="flex flex-wrap gap-2">
              {#each COUNTRIES as c}
                <button type="button" onclick={() => toggleCountry(c.code)} class="chip {countries.includes(c.code) ? 'chip-on' : ''}">
                  <span>{c.flag}</span>{c.label}
                </button>
              {/each}
            </div>
          </div>
        </div>

        <div class="session-row">
          <div>
            <p class="field-label">الجلسة الحالية</p>
            <p class="font-semibold">{browserStatus?.running ? `تعمل على ${browserStatus.profile_label}` : "لا توجد جلسة مفتوحة"}</p>
            {#if browserMessage}<p class="text-[12px]" style="color: var(--text-2)">{browserMessage}</p>{/if}
          </div>
          <div class="flex flex-wrap gap-2">
            <button class="btn btn-ghost btn-small" onclick={() => openOfficialView(selectedPlatform)}><Icon name="globe" size={15} /> فتح الجلسة</button>
            <button class="btn btn-ghost btn-small" disabled={!browserStatus?.running} onclick={stopIsolatedBrowser}><Icon name="x" size={15} /> إغلاق</button>
          </div>
        </div>

        <details class="advanced-box">
          <summary>إدارة بيانات الدخول والربط الرسمي</summary>
          <div class="grid grid-cols-1 md:grid-cols-[150px_1fr_1fr_auto] gap-3 mt-4">
            <select class="field-input" bind:value={vaultPlatform}>
              <option value="instagram">Instagram</option>
              <option value="tiktok">TikTok</option>
            </select>
            <input class="field-input" bind:value={vaultUsername} placeholder="@username" autocomplete="username" />
            <input class="field-input" bind:value={vaultPassword} placeholder="كلمة المرور" type="password" autocomplete="current-password" />
            <button class="btn btn-primary" onclick={saveVaultLogin}><Icon name="check" size={18} /> حفظ</button>
          </div>
          {#if savedLogins.length}
            <div class="list-stack mt-4">
              {#each savedLogins as item}
                <div class="list-row">
                  <div>
                    <p class="font-medium">{item.platform} · @{item.username}</p>
                    <p class="text-[12px]" style="color: var(--text-2)">Profile: {item.profile_label}</p>
                  </div>
                  <div class="flex gap-2">
                    <button class="btn btn-ghost btn-small" onclick={() => openSavedSession(item.id)}><Icon name="globe" size={14} /> فتح</button>
                    <button class="btn btn-ghost btn-small" onclick={() => deleteVaultLogin(item.id)}><Icon name="x" size={14} /> حذف</button>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
          <div class="divider my-4"></div>
          <div class="grid grid-cols-1 md:grid-cols-[160px_1fr_auto] gap-3">
            <select class="field-input" bind:value={selectedPlatform}>
              <option value="instagram">Instagram</option>
              <option value="tiktok">TikTok</option>
            </select>
            <input class="field-input" bind:value={oauthCode} placeholder="OAuth code" />
            <div class="flex gap-2">
              <button class="btn btn-ghost btn-small" onclick={() => beginOAuth(selectedPlatform)}><Icon name="globe" size={15} /> ربط</button>
              <button class="btn btn-primary btn-small" onclick={completeOAuth}><Icon name="check" size={15} /> حفظ</button>
            </div>
          </div>
          {#if vaultMessage}<p class="mt-3 text-[13px]" style="color: var(--success)">{vaultMessage}</p>{/if}
          {#if vaultError}<p class="mt-3 text-[13px]" style="color: var(--danger)">{vaultError}</p>{/if}
          {#if oauthMessage}<p class="mt-3 text-[13px]" style="color: var(--success)">{oauthMessage}</p>{/if}
          {#if oauthError}<p class="mt-3 text-[13px]" style="color: var(--danger)">{oauthError}</p>{/if}
          <p class="mt-3 text-[12px]" style="color: var(--text-2)">الربط الرسمي: {connectedLabel}</p>
        </details>
      </section>

      <section class="surface step-card">
        <div class="step-heading">
          <span class="step-number">3</span>
          <div>
            <h2>قراءة الوسائط واعتماد الوصف</h2>
            <p>اختر الصورة أو الفيديو، ثم دع Qwen2-VL يقرأها. راجع الوصف وعدّله قبل إرساله إلى Hermes.</p>
          </div>
        </div>
        <div class="grid grid-cols-1 gap-4 mt-5">
          <div>
            <label class="field-label" for="media">الصورة أو الفيديو</label>
            <div class="grid grid-cols-1 md:grid-cols-[1fr_auto] gap-3">
              <input id="media" class="field-input" bind:value={mediaPath} placeholder="اختياري: ملف صورة أو فيديو" readonly />
              <button class="btn btn-ghost" onclick={chooseMedia}><Icon name="download" size={18} /> اختيار ملف</button>
            </div>
          </div>
          <div>
            <div class="action-row mb-3">
              <div>
                <label class="field-label" for="vision-note">الوصف الذي سيرسله Vision إلى Hermes</label>
                <p class="text-[12px]" style="color: var(--text-2)">{visionSourceNote || "لم تتم قراءة الوسائط بعد."}</p>
              </div>
              <div class="flex flex-wrap gap-2">
                <button class="btn btn-primary btn-small" disabled={visionInspecting || !mediaPath.trim()} onclick={inspectMedia}>
                  {#if visionInspecting}<Icon name="spinner" size={15} class="animate-spin" /> قراءة الوسائط{:else}<Icon name="sparkle" size={15} /> قراءة الوسائط{/if}
                </button>
                <button class="btn btn-ghost btn-small" disabled={!visionDraft.trim()} onclick={approveVisionDescription}>
                  <Icon name="check" size={15} /> موافقة
                </button>
              </div>
            </div>
            <textarea
              id="vision-note"
              class="field-input min-h-[160px]"
              bind:value={visionDraft}
              oninput={() => { visionApproved = false; assistantResult = null; }}
              placeholder="سيظهر هنا وصف النموذج البصري. يمكنك تعديله قبل الموافقة."
            ></textarea>
            {#if visionApproved}<p class="mt-2 text-[13px]" style="color: var(--success)">تم اعتماد الوصف وسيرسل إلى Hermes.</p>{/if}
            {#if visionInspectError}<p class="mt-2 text-[13px]" style="color: var(--danger)">{visionInspectError}</p>{/if}
          </div>
        </div>
      </section>

      <section class="surface step-card">
        <div class="step-heading">
          <span class="step-number">4</span>
          <div>
            <h2>تحليل بالأدلة الفعلية</h2>
            <p>هذا هو البديل الأساسي لتحليل الاستراتيجية: وصف Vision المعتمد + منشورك إن وجد + أفضل منشورات منافسة بأرقام فعلية.</p>
          </div>
        </div>
        <div class="grid grid-cols-1 gap-4 mt-5">
          <div>
            <label class="field-label" for="own-post">رابط منشور من حسابك لجلب أرقامه الفعلية (اختياري للحساب التجريبي بلا منشورات)</label>
            <div class="grid grid-cols-1 md:grid-cols-[1fr_auto] gap-3">
              <input id="own-post" class="field-input" bind:value={fetchPostUrl} disabled={newAccountApproved} placeholder="رابط منشور مباشر: /video/ أو /reel/، وليس رابط الحساب" oninput={() => { fetchedInsights = null; fetchError = ""; }} />
              <button class="btn btn-ghost" disabled={fetchWorking || !fetchPostUrl.trim() || newAccountApproved} onclick={fetchInsights}>
                {#if fetchWorking}<Icon name="spinner" size={18} class="animate-spin" /> جارٍ الجلب{:else}<Icon name="chart" size={18} /> جلب أرقام المنشور{/if}
              </button>
            </div>
            <p class="mt-2 text-[12px]" style="color: var(--text-2)">إذا كان الحساب جديداً ولا يحتوي منشورات، اترك الحقل فارغاً. سيحلل GrowBox الوسائط والمنافسين فقط بدون اختلاق أرقام للحساب.</p>
          </div>
          <div class="new-account-box">
            <label class="flex items-start gap-3 cursor-pointer">
              <input
                type="checkbox"
                class="mt-1"
                checked={newAccountChecked}
                onchange={(event) => toggleNewAccountMode((event.currentTarget as HTMLInputElement).checked)}
              />
              <span>
                <strong>الحساب جديد بلا منشورات</strong>
                <small>فعّل هذا الخيار إذا كان الحساب تجريبياً أو يبدأ من الصفر. عند الموافقة لن يحاول التطبيق سحب أرقام من منشور غير موجود، وسيصيغ Hermes خطة أول منشور قابلة للانتشار بسرعة اعتماداً على Vision والمنافسين.</small>
              </span>
            </label>
            <button class="btn btn-ghost btn-small" disabled={!newAccountChecked || newAccountApproved} onclick={approveNewAccountMode}>
              <Icon name="check" size={15} /> {newAccountApproved ? "تم الاعتماد" : "موافق: ابدأ من الصفر"}
            </button>
          </div>
          <div>
            <label class="field-label" for="competitors">روابط منشورات منافسة من نفس المجال</label>
            <textarea id="competitors" class="field-input min-h-[110px]" bind:value={competitorPostUrls} placeholder="ضع كل رابط في سطر. التطبيق يحللها ويختار أفضل 5 حسب التفاعل الفعلي."></textarea>
          </div>
        </div>
        <div class="action-row mt-5">
          <button class="btn btn-primary" disabled={evidenceWorking || !isReady || !visionApproved} onclick={runEvidenceAnalysis}>
            {#if evidenceWorking}<Icon name="spinner" size={18} class="animate-spin" /> جارٍ التحليل بالأدلة{:else}<Icon name="sparkle" size={18} /> جلب وتحليل بالأدلة{/if}
          </button>
          {#if !visionApproved}<span class="text-[13px]" style="color: var(--text-2)">اعتمد وصف Vision في خطوة 3 أولاً.</span>{/if}
          {#if assistantWorking && progressMsg}<span class="text-[13px]" style="color: var(--text-2)">{progressMsg}</span>{/if}
          {#if fetchError}<span class="text-[13px]" style="color: var(--warning)">{fetchError}</span>{/if}
          {#if assistantError}<span class="text-[13px]" style="color: var(--danger)">{assistantError}</span>{/if}
        </div>

        {#if assistantResult && activePreview}
          <div class="result-hero">
            <div>
              <p class="field-label">Growth Score</p>
              <p class="score-text">{assistantResult.success_score}</p>
            </div>
            <div>
              <p class="field-label">وقت النشر</p>
              <h3>{assistantResult.suggested_time}</h3>
              <p>{assistantResult.timezone}</p>
            </div>
            <div>
              <p class="field-label">ملخص القرار</p>
              <p>{assistantResult.summary}</p>
            </div>
          </div>

          <div class="preview-grid">
            {#each assistantResult.cards as card}
              <article class="preview-card">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <h3>{card.platform_info.name}</h3>
                    <p class="text-[12px]" style="color: var(--text-2)">{card.niche_context.account_type} · {card.niche_context.region}</p>
                  </div>
                  <span class="pill">{card.strategy_insights.success_score}/100</span>
                </div>
                <p class="whitespace-pre-wrap leading-8 mt-4">{card.content_payload.caption}</p>
                <div class="tag-cloud">
                  {#each [...card.content_payload.hashtags.velocity, ...card.content_payload.hashtags.relevance] as tag}
                    <span class="chip">{tag}</span>
                  {/each}
                </div>
                <div class="signal-grid">
                  <span style={strategicScoreStyle(card.virality_indicators.strategic_score)}>Growth {card.virality_indicators.strategic_score}</span>
                  <span>{card.virality_indicators.hook_strength}</span>
                  <span>{card.virality_indicators.predicted_trend_alignment}</span>
                </div>
                <div class="action-row mt-4">
                  <button class="btn btn-ghost btn-small" onclick={() => editCard(card)}><Icon name="refresh" size={15} /> تعديل</button>
                  <button class="btn btn-primary btn-small" disabled={working} onclick={() => approveCard(card)}>
                    {#if working}<Icon name="spinner" size={15} class="animate-spin" /> اعتماد{:else}<Icon name="check" size={15} /> اعتماد هذه النسخة{/if}
                  </button>
                </div>
              </article>
            {/each}
          </div>
        {/if}

        {#if fetchedInsights || competitorInsights.length}
          <div class="draft-panel">
            <h3>الأدلة التي ستدخل إلى Hermes</h3>
            {#if fetchedInsights && hasRealMetrics(fetchedInsights)}
            {@const n = fetchedInsights.normalized}
            <div class="metric-strip">
              <span>منشور الحساب · {fetchedInsights.platform}</span>
              {#if (n.reach ?? n.views) != null}<span>{((n.reach ?? n.views) ?? 0).toLocaleString("ar")} مشاهدة/وصول</span>{/if}
              {#if n.likes != null}<span>{(n.likes ?? 0).toLocaleString("ar")} إعجاب</span>{/if}
              {#if n.comments != null}<span>{(n.comments ?? 0).toLocaleString("ar")} تعليق</span>{/if}
              {#if n.engagement_rate != null}<span>{(n.engagement_rate ?? 0).toFixed(2)}% تفاعل</span>{/if}
            </div>
            {:else if fetchedInsights}
              <p class="mt-3 text-[13px]" style="color: var(--text-2)">لا توجد أرقام منشور من حسابك بعد. سيستخدم Hermes هذا كحساب جديد بلا baseline، ولن يخترع أرقاماً.</p>
              {#if newAccountApproved}
                <p class="mt-2 text-[13px]" style="color: var(--success)">تم اعتماد مسار البدء من الصفر: الهدف الآن تسريع انتشار أول منشور وبناء أول baseline للحساب.</p>
              {/if}
            {/if}
            {#if competitorInsights.length}
              <p class="mt-3 text-[13px]" style="color: var(--text-2)">تم جلب {competitorInsights.length} منشور منافس. Hermes يستلم أفضل 5 حسب معدل التفاعل والأرقام المتاحة.</p>
            {/if}
          </div>
        {/if}
      </section>

      <section class="surface step-card">
        <div class="step-heading">
          <span class="step-number">5</span>
          <div>
            <h2>اعتماد ونشر يدوي</h2>
            <p>الاعتماد يحفظ المسودة وينسخ النص. صفحة النشر لا تفتح إلا عند الضغط عليها حتى لا تضيع الجلسة أو العمل.</p>
          </div>
        </div>
        <div class="action-row mt-5">
          <button class="btn btn-primary" disabled={working || !assistantResult} onclick={generateDraft}>
            {#if working}<Icon name="spinner" size={18} class="animate-spin" /> جارٍ الاعتماد{:else}<Icon name="sparkle" size={18} /> اعتماد وتجهيز المسودة{/if}
          </button>
          {#if !assistantResult}<span class="text-[13px]" style="color: var(--text-2)">شغّل خطوة 4 أولاً.</span>{/if}
          {#if formError}<span class="text-[13px]" style="color: var(--danger)">{formError}</span>{/if}
        </div>
        {#if viewingContent}
          <div class="draft-panel">
            <div class="flex flex-wrap items-center justify-between gap-3 mb-4">
              <div>
                <h3>المسودة الجاهزة #{viewingId}</h3>
                <p>اختر الوسائط داخل المنصة، ثم الصق الكابشن المنسوخ.</p>
              </div>
              <div class="flex flex-wrap gap-2">
                <button class="btn btn-primary btn-small" onclick={openPublishPages}><Icon name="globe" size={16} /> فتح صفحة النشر</button>
                <button class="btn btn-ghost btn-small" onclick={copyDraft}><Icon name="check" size={16} /> نسخ</button>
                <button class="btn btn-ghost btn-small" onclick={downloadMarkdown}><Icon name="download" size={16} /> تنزيل</button>
              </div>
            </div>
            {#if copyMessage}<p class="mb-3 text-[13px]" style="color: var(--success)">{copyMessage}</p>{/if}
            <div class="report-body">{@html rendered}</div>
          </div>
        {/if}
      </section>

      <section class="surface step-card">
        <div class="step-heading">
          <span class="step-number">6</span>
          <div>
            <h2>متابعة تطور الحساب</h2>
            <p>تقرير صحة الاستراتيجية يقارن الأداء كل 7 أيام ويقترح الاستمرار أو تغيير المسار.</p>
          </div>
        </div>
        <div class="evolution-grid mt-5">
          <div><p class="field-label">الحالة</p><h3>{evolution?.active ? "التتبع يعمل" : "التتبع متوقف"}</h3></div>
          <div><p class="field-label">آخر تقرير</p><h3>{evolution?.last_run_at ? new Date(evolution.last_run_at).toLocaleDateString("ar") : "لا يوجد"}</h3></div>
          <div><p class="field-label">عدد التقارير</p><h3>{evolutionReports.length}</h3></div>
        </div>
        <div class="action-row mt-4">
          <button class="btn {evolution?.active ? 'btn-ghost' : 'btn-primary'}" onclick={toggleEvolutionTracker}>
            <Icon name={evolution?.active ? "x" : "calendar"} size={18} />
            {evolution?.active ? "إيقاف التتبع" : "تشغيل التتبع"}
          </button>
          <button class="btn btn-ghost" disabled={evolutionWorking} onclick={runEvolutionNow}>
            {#if evolutionWorking}<Icon name="spinner" size={18} class="animate-spin" /> جارٍ التقرير{:else}<Icon name="chart" size={18} /> تقرير الآن{/if}
          </button>
        </div>
        {#if evolutionError}<p class="mt-3 text-[13px]" style="color: var(--danger)">{evolutionError}</p>{/if}
        {#if evolution?.last_summary}<p class="mt-3 text-[13px]" style="color: var(--text-2)">{evolution.last_summary}</p>{/if}
        {#if evolutionRendered}<div class="draft-panel report-body">{@html evolutionRendered}</div>{/if}
      </section>

      <details class="advanced-box surface">
        <summary>الأدوات المتقدمة والسجل</summary>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
          <div>
            <h3 class="font-semibold mb-3">وضع الخلفية</h3>
            <div class="action-row">
              <button class="btn {background?.active ? 'btn-ghost' : 'btn-primary'}" onclick={toggleBackgroundMode}>
                <Icon name={background?.active ? "x" : "refresh"} size={18} />
                {background?.active ? "إيقاف الخلفية" : "تشغيل الخلفية"}
              </button>
              <button class="btn btn-ghost" onclick={sendToTray}><Icon name="download" size={18} /> تصغير للشريط</button>
            </div>
            {#if backgroundMsg}<p class="mt-3 text-[13px]" style="color: var(--text-2)">{backgroundMsg}</p>{/if}
            {#if backgroundError}<p class="mt-3 text-[13px]" style="color: var(--danger)">{backgroundError}</p>{/if}
          </div>
          <div>
            <h3 class="font-semibold mb-3">البوستات السابقة</h3>
            {#if history.length === 0}
              <p class="text-[14px]" style="color: var(--text-2)">لا توجد بوستات محفوظة بعد.</p>
            {:else}
              <div class="list-stack">
                {#each history as h}
                  <button onclick={() => viewReport(h.id)} class="list-row text-right">
                    <span class="font-medium">{h.client_name}</span>
                    <span class="text-[12px]" style="color: var(--text-2)">{new Date(h.generated_at).toLocaleDateString("ar")}</span>
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </details>
    {/if}
  </main>
  {#if working}
    <div class="loading-overlay" role="status" aria-live="polite">
      <div class="loading-panel">
        <div class="flex items-center gap-3 mb-4">
          <div class="loading-spinner"><Icon name="spinner" size={24} class="animate-spin" /></div>
          <div>
            <h2 class="text-[18px] font-semibold">{operationName || "اعتماد وتجهيز المسودة"}</h2>
            <p class="text-[13px]" style="color: var(--text-2)">ابقِ التطبيق مفتوحاً حتى تنتهي العملية.</p>
          </div>
        </div>
        <div class="progress-track"><div class="progress-fill" style="width: {draftProgressPercent(reportStage)}%"></div></div>
        <div class="time-grid mt-4">
          <div class="time-cell">
            <span>الوقت الفعلي</span>
            <strong>{operationElapsed}</strong>
          </div>
          <div class="time-cell">
            <span>المرحلة الحالية</span>
            <strong>{operationPhaseElapsed}</strong>
          </div>
        </div>
        <div class="flex items-center justify-between mt-3 text-[12px]" style="color: var(--text-2)">
          <span>{operationPhase || draftProgressLabel(reportStage)}</span>
          <span>{draftProgressPercent(reportStage)}%</span>
        </div>
        <p class="mt-4 text-[14px] leading-7">{progressMsg || "Hermes يجهز النسخة النهائية…"}</p>
        <p class="mt-2 text-[12px]" style="color: var(--text-2)">{operationMeasuredLine}</p>
      </div>
    </div>
  {/if}
  {#if pipelineBusy && !working}
    <div class="loading-overlay" role="status" aria-live="polite">
      <div class="loading-panel">
        <div class="flex items-center gap-3 mb-4">
          <div class="loading-spinner"><Icon name="spinner" size={24} class="animate-spin" /></div>
          <div>
            <h2 class="text-[18px] font-semibold">{operationName || "إدارة الموارد على الطلب"}</h2>
            <p class="text-[13px]" style="color: var(--text-2)">تم إيقاف التفاعل مؤقتاً حتى تنتهي المرحلة الحالية.</p>
          </div>
        </div>
        <div class="progress-track"><div class="progress-fill" style="width: {operationProgress ?? 64}%"></div></div>
        <div class="time-grid mt-4">
          <div class="time-cell">
            <span>الوقت الفعلي</span>
            <strong>{operationElapsed}</strong>
          </div>
          <div class="time-cell">
            <span>المرحلة الحالية</span>
            <strong>{operationPhaseElapsed}</strong>
          </div>
        </div>
        <div class="flex items-center justify-between mt-3 text-[12px]" style="color: var(--text-2)">
          <span>{operationPhase || pipelineBusyMessage}</span>
          {#if operationProgress != null}<span>{operationProgress}%</span>{/if}
        </div>
        <p class="mt-4 text-[14px] leading-7">{pipelineBusyMessage}</p>
        <p class="mt-2 text-[12px]" style="color: var(--text-2)">{operationMeasuredLine}</p>
      </div>
    </div>
  {/if}
</div>
