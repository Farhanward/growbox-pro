use crate::{data_fetcher, insights, llm, oauth, vision, ClientInput};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::AppHandle;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PublishingAssistantInput {
    pub client: ClientInput,
    pub platform: String,
    pub media_path: Option<String>,
    pub media_note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScoredHashtag {
    pub tag: String,
    pub velocity: u8,
    pub relevance: u8,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformPreview {
    pub platform: String,
    pub caption: String,
    pub hashtags: Vec<String>,
    pub suggested_time: String,
    pub visual_note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeoProfile {
    pub countries: Vec<String>,
    pub primary_country: String,
    pub timezone: String,
    pub locale: String,
    pub ip_alignment_note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformInfo {
    pub name: String,
    pub post_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NicheContext {
    pub account_type: String,
    pub region: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HashtagGroups {
    pub velocity: Vec<String>,
    pub relevance: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentPayload {
    pub caption: String,
    pub hashtags: HashtagGroups,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StrategyInsights {
    pub success_score: u8,
    pub posting_time: String,
    pub reasoning: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ViralityIndicators {
    pub hook_strength: String,
    pub shareability_factor: String,
    pub predicted_trend_alignment: String,
    pub strategic_score: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PublishingCard {
    pub platform_info: PlatformInfo,
    pub niche_context: NicheContext,
    pub content_payload: ContentPayload,
    pub strategy_insights: StrategyInsights,
    pub virality_indicators: ViralityIndicators,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PublishingAssistantOutput {
    pub summary: String,
    pub success_score: u8,
    pub suggested_time: String,
    pub timezone: String,
    pub geo_profile: GeoProfile,
    pub hashtags: Vec<ScoredHashtag>,
    pub instagram: PlatformPreview,
    pub tiktok: PlatformPreview,
    pub virality_indicators: ViralityIndicators,
    pub cards: Vec<PublishingCard>,
    pub rationale: Vec<String>,
    pub data_points: Vec<String>,
    pub guardrails: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AiPlatformCopy {
    caption: Option<String>,
    hashtags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AiCopy {
    platform_info: Option<PlatformInfo>,
    niche_context: Option<NicheContext>,
    content_payload: Option<ContentPayload>,
    strategy_insights: Option<StrategyInsights>,
    virality_indicators: Option<ViralityIndicators>,
    summary: Option<String>,
    success_score: Option<u8>,
    suggested_time: Option<String>,
    instagram: Option<AiPlatformCopy>,
    tiktok: Option<AiPlatformCopy>,
    visual_note: Option<String>,
    rationale: Option<Vec<String>>,
}

struct CountryRule {
    code: &'static str,
    label: &'static str,
    marker_terms: &'static [&'static str],
    weekday_slots: &'static [&'static str],
    weekend_slots: &'static [&'static str],
    timezone: &'static str,
    locale: &'static str,
}

struct NicheRule {
    canonical: &'static str,
    keywords: &'static [&'static str],
    blocked_terms: &'static [&'static str],
    fallback_tags: &'static [&'static str],
}

const COUNTRIES: &[CountryRule] = &[
    CountryRule {
        code: "SA",
        label: "السعودية",
        marker_terms: &["sa", "ksa", "riyadh", "jeddah", "السعودية", "الرياض", "جده", "جدة"],
        weekday_slots: &["20:45", "21:30", "22:15"],
        weekend_slots: &["16:30", "21:15", "22:30"],
        timezone: "Asia/Riyadh",
        locale: "ar-SA",
    },
    CountryRule {
        code: "AE",
        label: "الإمارات",
        marker_terms: &["ae", "uae", "dubai", "abudhabi", "الإمارات", "دبي", "أبوظبي"],
        weekday_slots: &["19:45", "20:30", "21:45"],
        weekend_slots: &["15:45", "20:15", "21:30"],
        timezone: "Asia/Dubai",
        locale: "ar-AE",
    },
    CountryRule {
        code: "KW",
        label: "الكويت",
        marker_terms: &["kw", "q8", "kuwait", "الكويت"],
        weekday_slots: &["20:30", "21:45", "22:30"],
        weekend_slots: &["16:15", "21:00", "22:15"],
        timezone: "Asia/Kuwait",
        locale: "ar-KW",
    },
    CountryRule {
        code: "QA",
        label: "قطر",
        marker_terms: &["qa", "qatar", "doha", "قطر", "الدوحة"],
        weekday_slots: &["19:45", "20:45", "22:00"],
        weekend_slots: &["16:00", "20:30", "22:00"],
        timezone: "Asia/Qatar",
        locale: "ar-QA",
    },
    CountryRule {
        code: "BH",
        label: "البحرين",
        marker_terms: &["bh", "bahrain", "البحرين", "manama"],
        weekday_slots: &["20:00", "21:15", "22:00"],
        weekend_slots: &["16:00", "20:45", "22:00"],
        timezone: "Asia/Bahrain",
        locale: "ar-BH",
    },
    CountryRule {
        code: "OM",
        label: "عُمان",
        marker_terms: &["om", "oman", "muscat", "عمان", "عُمان", "مسقط"],
        weekday_slots: &["19:30", "20:45", "21:45"],
        weekend_slots: &["15:30", "20:00", "21:30"],
        timezone: "Asia/Muscat",
        locale: "ar-OM",
    },
];

const NICHES: &[NicheRule] = &[
    NicheRule {
        canonical: "تصوير",
        keywords: &["تصوير", "مصور", "فوتو", "فوتوغرافي", "كاميرا", "لايتروم", "بورتريه", "منتجات", "اعراس", "photography", "photo", "lens", "camera"],
        blocked_terms: &["food", "مطاعم", "وصفات", "طبخ", "gaming", "fitness", "رياضة", "مكياج", "beauty", "fashion"],
        fallback_tags: &["#تصوير_احترافي", "#تصوير_منتجات", "#فوتوغرافي", "#مصورين_الخليج", "#كاميرا"],
    },
    NicheRule {
        canonical: "طعام",
        keywords: &["طعام", "مطعم", "اكل", "أكل", "وصفات", "قهوة", "كافيه", "حلويات", "فطور", "عشاء", "food", "foodie", "cafe", "restaurant"],
        blocked_terms: &["fashion", "موضة", "أزياء", "gaming", "fitness", "photography", "مصور", "تعليم"],
        fallback_tags: &["#مطاعم_الخليج", "#اكلات", "#كافيهات", "#قهوة", "#foodie_gulf"],
    },
    NicheRule {
        canonical: "موضة",
        keywords: &["موضة", "ملابس", "أزياء", "فاشن", "ستايل", "لوك", "عبايات", "فساتين", "تنسيق", "fashion", "style", "ootd"],
        blocked_terms: &["food", "مطاعم", "وصفات", "طبخ", "gaming", "fitness", "رياضة", "تعليم", "photography"],
        fallback_tags: &["#موضة_خليجية", "#ستايل", "#عبايات", "#لوك_اليوم", "#تنسيق"],
    },
    NicheRule {
        canonical: "تجميل",
        keywords: &["تجميل", "ميك", "مكياج", "بشرة", "شعر", "عطور", "مناكير", "beauty", "makeup", "skin", "hair"],
        blocked_terms: &["food", "مطاعم", "gaming", "fitness", "تعليم", "سفر"],
        fallback_tags: &["#تجميل_خليجي", "#مكياج", "#عناية_بشرة", "#ميك_اب", "#beauty_arabia"],
    },
    NicheRule {
        canonical: "سفر",
        keywords: &["سفر", "سياحة", "رحلات", "فنادق", "وجهات", "مغامرة", "travel", "trip", "hotel"],
        blocked_terms: &["food", "مطاعم", "fashion", "مكياج", "fitness", "تعليم"],
        fallback_tags: &["#سفر", "#سياحة_خليجية", "#رحلات", "#وجهات_سياحية", "#travel_arabia"],
    },
    NicheRule {
        canonical: "رياضة",
        keywords: &["رياضة", "لياقة", "جيم", "تمارين", "مدرب", "تغذية", "fitness", "gym", "workout", "padel"],
        blocked_terms: &["food", "مطاعم", "fashion", "مكياج", "سفر", "تعليم"],
        fallback_tags: &["#لياقة", "#تمارين", "#جيم", "#مدرب_شخصي", "#fitness_gulf"],
    },
    NicheRule {
        canonical: "تعليم",
        keywords: &["تعليم", "طلاب", "دورات", "مهارات", "قدرات", "تحصيلي", "برمجة", "قراءة", "study", "course"],
        blocked_terms: &["food", "مطاعم", "fashion", "gaming", "fitness", "مكياج"],
        fallback_tags: &["#تعليم", "#دورات", "#مهارات", "#تعليم_عن_بعد", "#study_arabia"],
    },
    NicheRule {
        canonical: "ألعاب",
        keywords: &["ألعاب", "قيمنق", "قيمر", "ببجي", "فورتنايت", "فالورنت", "فيفا", "gaming", "esports", "streamer"],
        blocked_terms: &["food", "مطاعم", "fashion", "مكياج", "fitness", "تعليم"],
        fallback_tags: &["#قيمنق", "#ألعاب", "#قيمر_عربي", "#esports_gulf", "#gaming_arabia"],
    },
];

pub async fn analyze<F>(
    app: &AppHandle,
    input: PublishingAssistantInput,
    on_stage: F,
) -> Result<PublishingAssistantOutput>
where
    F: Fn(&str, &str) + Send + Sync + Clone + 'static,
{
    if input.media_note.trim().is_empty() {
        return Err(anyhow!("أدخل وصف الصورة أو الفيديو قبل تشغيل مساعد النشر."));
    }
    if input.client.countries.is_empty() {
        return Err(anyhow!("اختر دولة واحدة على الأقل حتى يعمل الاستهداف الجغرافي."));
    }

    on_stage("assistant", "فلترة النيتش والدول المستهدفة…");
    let rule = niche_rule(&input.client.niche);
    let selected_countries = selected_countries(&input.client.countries);

    let bundle = insights::gather(app, &input.client, on_stage.clone()).await?;
    let account_data = None;
    let scored = rank_hashtags(&bundle.trending_hashtags, rule, &selected_countries);
    let deterministic_time = best_time(&input.platform, &selected_countries, !bundle.best_times_weekend.is_empty());
    let allowed: Vec<String> = scored.iter().map(|h| h.tag.clone()).collect();

    on_stage("vision", "تحليل الوسائط محلياً عبر Vision Agent…");
    let media_note = enrich_media_note(app, input.media_path.as_deref(), &input.media_note).await;

    on_stage("assistant", "صياغة كابشن المنصات بالنموذج المحلي…");
    let context = build_strategy_context(&input, &bundle, account_data.as_ref(), &allowed, &deterministic_time, rule);
    let ai = llm::generate_publishing_assistant(&input.client, &input.platform, &context, &media_note, &allowed, &deterministic_time)
        .await
        .ok()
        .and_then(|text| parse_ai_copy(&text));

    let instagram_caption = ai
        .as_ref()
        .and_then(|x| x.instagram.as_ref())
        .and_then(|x| x.caption.clone())
        .unwrap_or_else(|| fallback_caption("Instagram", &input, &selected_countries));
    let tiktok_caption = ai
        .as_ref()
        .and_then(|x| x.tiktok.as_ref())
        .and_then(|x| x.caption.clone())
        .unwrap_or_else(|| fallback_caption("TikTok", &input, &selected_countries));
    let visual_note = ai
        .as_ref()
        .and_then(|x| x.visual_note.clone())
        .unwrap_or_else(|| "راجع أول ثانيتين، وضوح المنتج/الفكرة، ووجود دعوة تفاعل مباشرة قبل الاعتماد.".into());
    let summary = ai
        .as_ref()
        .and_then(|x| x.summary.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "استراتيجية {} لـ {} في {} جاهزة: وسوم مصفّاة حسب النيتش والدولة، مع توقيت نشر مقترح ونسختين للكابشن.",
                rule.canonical,
                input.client.name,
                selected_countries.iter().map(|c| c.label).collect::<Vec<_>>().join("، ")
            )
        });

    let deterministic_score = success_score(&input, &bundle, &scored, account_data.is_some());
    let score = ai
        .as_ref()
        .and_then(|x| x.success_score)
        .map(|score| score.clamp(1, 100))
        .unwrap_or(deterministic_score);
    let suggested_time = ai
        .as_ref()
        .and_then(|x| x.suggested_time.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| deterministic_time.clone());
    let instagram_tags = sanitize_model_tags(
        ai.as_ref().and_then(|x| x.instagram.as_ref()).and_then(|x| x.hashtags.clone()),
        &allowed,
        16,
    );
    let tiktok_tags = sanitize_model_tags(
        ai.as_ref().and_then(|x| x.tiktok.as_ref()).and_then(|x| x.hashtags.clone()),
        &allowed,
        10,
    );
    let rationale = ai
        .as_ref()
        .and_then(|x| {
            x.strategy_insights
                .as_ref()
                .map(|insights| vec![insights.reasoning.clone()])
        })
        .filter(|v| !v.is_empty())
        .or_else(|| {
            ai.as_ref()
                .and_then(|x| x.rationale.clone())
                .filter(|v| !v.is_empty())
        })
        .unwrap_or_else(|| default_rationale(rule, &selected_countries, account_data.is_some()));
    let fallback_virality = virality_indicators(score, &media_note, &scored);
    let virality_indicators = ai
        .as_ref()
        .and_then(|x| x.virality_indicators.clone())
        .map(|value| normalize_virality_indicators(value, &fallback_virality))
        .unwrap_or_else(|| fallback_virality.clone());
    let selected_platform_card = ai
        .as_ref()
        .and_then(|x| exact_model_card(
            x,
            &input,
            rule,
            &selected_countries,
            &allowed,
            score,
            &suggested_time,
            &virality_indicators,
        ));
    let geo_profile = geo_profile(&selected_countries);
    let instagram_preview = PlatformPreview {
        platform: "Instagram".into(),
        caption: instagram_caption,
        hashtags: instagram_tags,
        suggested_time: suggested_time.clone(),
        visual_note: visual_note.clone(),
    };
    let tiktok_preview = PlatformPreview {
        platform: "TikTok".into(),
        caption: tiktok_caption,
        hashtags: tiktok_tags,
        suggested_time: suggested_time.clone(),
        visual_note,
    };
    let cards = build_cards(
        &input,
        rule,
        &selected_countries,
        &scored,
        &instagram_preview,
        &tiktok_preview,
        score,
        &suggested_time,
        &virality_indicators,
        selected_platform_card,
        &rationale,
    );

    Ok(PublishingAssistantOutput {
        summary,
        success_score: score,
        suggested_time: suggested_time.clone(),
        timezone: geo_profile.timezone.clone(),
        geo_profile,
        hashtags: scored.clone(),
        instagram: instagram_preview,
        tiktok: tiktok_preview,
        virality_indicators,
        cards,
        rationale,
        data_points: data_points(&bundle, account_data.as_ref()),
        guardrails: vec![
            "لا يتم اقتراح هاشتاق خارج تقاطع النيتش والدول المحددة.".into(),
            "التحليل يعمل محلياً، وقرار النشر النهائي يبقى بيد المستخدم.".into(),
            "بيانات الحساب المرتبط تُستخدم كإشارة تحليل فقط ولا يتم إرسالها لخدمة خارجية.".into(),
        ],
    })
}

fn niche_rule(niche: &str) -> &'static NicheRule {
    let canonical = match niche.trim() {
        "مصور" | "مصور فوتوغرافي" | "استوديو تصوير" => "تصوير",
        "مطعم" | "كافيه" | "مقهى" | "حلويات" => "طعام",
        "متجر ملابس" | "عبايات" | "أزياء" | "فاشن" => "موضة",
        other => other,
    };
    NICHES
        .iter()
        .find(|rule| rule.canonical == canonical)
        .unwrap_or(&NICHES[0])
}

fn selected_countries(codes: &[String]) -> Vec<&'static CountryRule> {
    let selected: Vec<&'static CountryRule> = COUNTRIES.iter().filter(|c| codes.iter().any(|code| code == c.code)).collect();
    if selected.is_empty() {
        vec![&COUNTRIES[0]]
    } else {
        selected
    }
}

fn rank_hashtags(source: &[String], rule: &NicheRule, countries: &[&CountryRule]) -> Vec<ScoredHashtag> {
    let mut seen = HashSet::new();
    let mut tags: Vec<String> = source.to_vec();
    tags.extend(rule.fallback_tags.iter().map(|s| (*s).to_string()));
    for country in countries {
        tags.push(format!("#{}_{}", rule.canonical.replace(' ', "_"), country.label.replace(' ', "_")));
    }

    let mut scored = vec![];
    for tag in tags {
        let normalized = tag.to_lowercase();
        if !seen.insert(normalized.clone()) || rule.blocked_terms.iter().any(|term| normalized.contains(&term.to_lowercase())) {
            continue;
        }
        let relevance_hits = rule.keywords.iter().filter(|term| normalized.contains(&term.to_lowercase())).count() as u8;
        let country_hits = countries
            .iter()
            .filter(|country| country.marker_terms.iter().any(|term| normalized.contains(&term.to_lowercase())))
            .count() as u8;
        let broad_gulf = normalized.contains("gulf") || normalized.contains("خليج") || normalized.contains("arabia");
        if relevance_hits == 0 && country_hits == 0 && !broad_gulf {
            continue;
        }
        let relevance = (58 + relevance_hits.saturating_mul(11) + country_hits.saturating_mul(8) + u8::from(broad_gulf) * 5).min(99);
        let velocity = (50 + country_hits.saturating_mul(14) + relevance_hits.saturating_mul(7) + u8::from(broad_gulf) * 8).min(96);
        scored.push(ScoredHashtag {
            tag,
            velocity,
            relevance,
            reason: if country_hits > 0 {
                "مرتبط بالدولة والنيتش".into()
            } else if broad_gulf {
                "مرتبط بالسوق الخليجي والنيتش".into()
            } else {
                "مرتبط مباشرة بنشاط الحساب".into()
            },
        });
    }
    scored.sort_by_key(|h| std::cmp::Reverse(h.velocity as u16 + h.relevance as u16));
    scored.truncate(20);
    scored
}

fn best_time(platform: &str, countries: &[&CountryRule], prefer_weekend: bool) -> String {
    let country = countries.first().copied().unwrap_or(&COUNTRIES[0]);
    let slots = if prefer_weekend { country.weekend_slots } else { country.weekday_slots };
    let idx = if platform == "tiktok" { 1 } else { 0 };
    slots.get(idx).unwrap_or(&"21:00").to_string()
}

fn success_score(input: &PublishingAssistantInput, bundle: &insights::InsightBundle, tags: &[ScoredHashtag], has_account_data: bool) -> u8 {
    let mut score: i32 = 45;
    score += (tags.len().min(16) as i32) * 2;
    if input.media_note.chars().count() > 80 {
        score += 8;
    }
    if !bundle.client_profiles.is_empty() {
        score += 6;
    }
    if !bundle.competitor_handles.is_empty() {
        score += 5;
    }
    if has_account_data {
        score += 8;
    }
    if input.client.countries.len() <= 2 {
        score += 4;
    }
    score.clamp(1, 100) as u8
}

fn build_strategy_context(
    input: &PublishingAssistantInput,
    bundle: &insights::InsightBundle,
    account_data: Option<&oauth::AccountData>,
    hashtags: &[String],
    suggested_time: &str,
    rule: &NicheRule,
) -> String {
    let mut s = insights::build_context_block(bundle, &input.client);
    s.push_str("\n## مصفوفة مساعد النشر الذكي\n");
    s.push_str(&format!("- النيتش المعتمد: {}\n", rule.canonical));
    s.push_str(&format!("- المنصة الحالية: {}\n", input.platform));
    s.push_str(&format!("- وقت النشر المقترح: {} بتوقيت الخليج\n", suggested_time));
    s.push_str(&format!("- الهاشتاقات المسموحة فقط: {}\n", hashtags.join(" ")));
    if let Some(account) = account_data {
        s.push_str("\n## بيانات الحساب المرتبط\n");
        s.push_str(&format!("- {}: {}\n", account.platform, account.summary));
    }
    s
}

async fn enrich_media_note(app: &AppHandle, media_path: Option<&str>, media_note: &str) -> String {
    match vision::describe_media(app, media_path).await {
        Ok(Some(vision_note)) => format!(
            "{}\n\n{}",
            media_note.trim(),
            vision_note.trim()
        ),
        _ => media_note.trim().to_string(),
    }
}

fn parse_ai_copy(text: &str) -> Option<AiCopy> {
    let trimmed = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(parsed) = serde_json::from_str::<AiCopy>(trimmed) {
        return Some(parsed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    serde_json::from_str::<AiCopy>(&trimmed[start..=end]).ok()
}

fn fallback_caption(_platform: &str, input: &PublishingAssistantInput, countries: &[&CountryRule]) -> String {
    let country_line = countries.iter().map(|c| c.label).collect::<Vec<_>>().join("، ");
    format!(
        "{} | {} في {}.\n{}\nاكتبوا لنا رأيكم بالتعليقات، واحفظوا المنشور للرجوع له لاحقاً.",
        input.client.name,
        input.client.niche,
        country_line,
        input.media_note.trim()
    )
}

fn default_rationale(rule: &NicheRule, countries: &[&CountryRule], has_account_data: bool) -> Vec<String> {
    let mut out = vec![
        format!("تم تثبيت النيتش على {} قبل اختيار الوسوم.", rule.canonical),
        format!("تم حصر الترندات داخل {}", countries.iter().map(|c| c.label).collect::<Vec<_>>().join("، ")),
        "تمت موازنة سرعة صعود الوسم مع صلته بنشاط الحساب.".into(),
    ];
    if has_account_data {
        out.push("تمت إضافة بيانات الحساب المرتبط كإشارة واقعية للقرار.".into());
    }
    out
}

fn data_points(bundle: &insights::InsightBundle, account_data: Option<&oauth::AccountData>) -> Vec<String> {
    let mut points = vec![
        format!("حسابات عامة محللة: {}", bundle.client_profiles.len()),
        format!("منافسون مرجعيون ضمن النيتش: {}", bundle.competitor_handles.len()),
        format!("هاشتاقات مصدرية قبل الفلترة: {}", bundle.trending_hashtags.len()),
    ];
    if let Some(account) = account_data {
        points.push(format!("بيانات حساب مرتبط: {}", account.summary));
    }
    points
}

fn sanitize_model_tags(model_tags: Option<Vec<String>>, allowed: &[String], limit: usize) -> Vec<String> {
    let allowed_set: HashSet<&str> = allowed.iter().map(String::as_str).collect();
    let mut tags = vec![];
    if let Some(model_tags) = model_tags {
        for tag in model_tags {
            if allowed_set.contains(tag.as_str()) && !tags.contains(&tag) {
                tags.push(tag);
            }
            if tags.len() >= limit {
                return tags;
            }
        }
    }
    for tag in allowed.iter().take(limit) {
        if !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    tags
}

fn geo_profile(countries: &[&CountryRule]) -> GeoProfile {
    let primary = countries.first().copied().unwrap_or(&COUNTRIES[0]);
    GeoProfile {
        countries: countries.iter().map(|c| c.label.to_string()).collect(),
        primary_country: primary.label.into(),
        timezone: primary.timezone.into(),
        locale: primary.locale.into(),
        ip_alignment_note: "يتم استخدام الدولة المختارة لضبط التوقيت واللهجة وقرارات الترند. لا يغيّر التطبيق عنوان IP ولا يحاكي بصمة جهاز.".into(),
    }
}

fn exact_model_card(
    ai: &AiCopy,
    input: &PublishingAssistantInput,
    rule: &NicheRule,
    countries: &[&CountryRule],
    allowed: &[String],
    score: u8,
    suggested_time: &str,
    fallback_virality: &ViralityIndicators,
) -> Option<PublishingCard> {
    let mut card = PublishingCard {
        platform_info: ai.platform_info.clone().unwrap_or_else(|| PlatformInfo {
            name: input.platform.clone(),
            post_id: format!("GBX-{}", uuid::Uuid::new_v4().simple()),
        }),
        niche_context: ai.niche_context.clone().unwrap_or_else(|| NicheContext {
            account_type: rule.canonical.into(),
            region: countries.iter().map(|c| c.label).collect::<Vec<_>>().join("، "),
        }),
        content_payload: ai.content_payload.clone()?,
        strategy_insights: ai.strategy_insights.clone().unwrap_or_else(|| StrategyInsights {
            success_score: score,
            posting_time: suggested_time.into(),
            reasoning: "تمت مطابقة النيتش والدولة ثم فلترة الوسوم غير المتوافقة.".into(),
        }),
        virality_indicators: ai
            .virality_indicators
            .clone()
            .map(|value| normalize_virality_indicators(value, fallback_virality))
            .unwrap_or_else(|| fallback_virality.clone()),
    };
    card.content_payload.hashtags.velocity = sanitize_model_tags(
        Some(card.content_payload.hashtags.velocity.clone()),
        allowed,
        8,
    );
    card.content_payload.hashtags.relevance = sanitize_model_tags(
        Some(card.content_payload.hashtags.relevance.clone()),
        allowed,
        8,
    );
    card.strategy_insights.success_score = card.strategy_insights.success_score.clamp(1, 100);
    Some(card)
}

fn build_cards(
    input: &PublishingAssistantInput,
    rule: &NicheRule,
    countries: &[&CountryRule],
    scored: &[ScoredHashtag],
    instagram: &PlatformPreview,
    tiktok: &PlatformPreview,
    score: u8,
    suggested_time: &str,
    virality_indicators: &ViralityIndicators,
    selected_platform_card: Option<PublishingCard>,
    rationale: &[String],
) -> Vec<PublishingCard> {
    let mut cards = vec![];
    for preview in [instagram, tiktok] {
        if let Some(model_card) = selected_platform_card.as_ref().filter(|card| {
            card.platform_info.name.eq_ignore_ascii_case(&preview.platform)
                || card.platform_info.name.eq_ignore_ascii_case(&input.platform)
        }) {
            cards.push(model_card.clone());
            continue;
        }
        cards.push(PublishingCard {
            platform_info: PlatformInfo {
                name: preview.platform.clone(),
                post_id: format!("GBX-{}-{}", preview.platform.to_uppercase(), uuid::Uuid::new_v4().simple()),
            },
            niche_context: NicheContext {
                account_type: rule.canonical.into(),
                region: countries.iter().map(|c| c.label).collect::<Vec<_>>().join("، "),
            },
            content_payload: ContentPayload {
                caption: preview.caption.clone(),
                hashtags: HashtagGroups {
                    velocity: top_tags_by(scored, |tag| tag.velocity, 8),
                    relevance: top_tags_by(scored, |tag| tag.relevance, 8),
                },
            },
            strategy_insights: StrategyInsights {
                success_score: score,
                posting_time: suggested_time.into(),
                reasoning: rationale.first().cloned().unwrap_or_else(|| {
                    "تمت مطابقة الهاشتاقات مع النيتش والدولة ووقت نشاط الجمهور.".into()
                }),
            },
            virality_indicators: virality_indicators.clone(),
        });
    }
    cards
}

fn virality_indicators(score: u8, media_note: &str, scored: &[ScoredHashtag]) -> ViralityIndicators {
    let note = media_note.trim();
    let has_direct_hook = note.contains('!') || note.contains('؟') || note.contains('?');
    let hook_strength = if score >= 82 && (note.chars().count() >= 80 || has_direct_hook) {
        "High"
    } else if score >= 65 || note.chars().count() >= 50 {
        "Medium"
    } else {
        "Low"
    };

    let shareability_factor = match score {
        86..=100 => "Top 10%",
        74..=85 => "Top 25%",
        60..=73 => "Top 40%",
        _ => "Needs Work",
    };

    let max_velocity = scored.iter().map(|tag| tag.velocity).max().unwrap_or(0);
    let predicted_trend_alignment = if max_velocity >= 86 {
        "Emerging"
    } else if max_velocity >= 68 {
        "Rising"
    } else {
        "Stable"
    };

    ViralityIndicators {
        hook_strength: hook_strength.into(),
        shareability_factor: shareability_factor.into(),
        predicted_trend_alignment: predicted_trend_alignment.into(),
        strategic_score: llm::calculate_strategic_score(hook_strength, shareability_factor, predicted_trend_alignment),
    }
}

fn normalize_virality_indicators(value: ViralityIndicators, fallback: &ViralityIndicators) -> ViralityIndicators {
    let hook_strength = normalize_choice(&value.hook_strength, &["High", "Medium", "Low"], &fallback.hook_strength);
    let shareability_factor = normalize_choice(
        &value.shareability_factor,
        &["High", "Medium", "Low", "Top 10%", "Top 25%", "Top 40%", "Needs Work"],
        &fallback.shareability_factor,
    );
    let predicted_trend_alignment = normalize_choice(
        &value.predicted_trend_alignment,
        &["Peaking", "Emerging", "Rising", "Stable", "High", "Medium", "Low"],
        &fallback.predicted_trend_alignment,
    );
    let strategic_score = llm::calculate_strategic_score(
        &hook_strength,
        &shareability_factor,
        &predicted_trend_alignment,
    );
    ViralityIndicators { hook_strength, shareability_factor, predicted_trend_alignment, strategic_score }
}

fn normalize_choice(value: &str, allowed: &[&str], fallback: &str) -> String {
    allowed
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(value.trim()))
        .map(|candidate| (*candidate).to_string())
        .unwrap_or_else(|| fallback.to_string())
}

fn top_tags_by<F>(scored: &[ScoredHashtag], key: F, limit: usize) -> Vec<String>
where
    F: Fn(&ScoredHashtag) -> u8,
{
    let mut tags = scored.to_vec();
    tags.sort_by_key(|tag| std::cmp::Reverse(key(tag)));
    tags.into_iter().take(limit).map(|tag| tag.tag).collect()
}

/// Same as `analyze` but injects fetched insights as an additional static context block.
pub async fn analyze_with_insights<F>(
    app: &AppHandle,
    input: PublishingAssistantInput,
    fetched: data_fetcher::CapturedInsights,
    on_stage: F,
) -> Result<PublishingAssistantOutput>
where
    F: Fn(&str, &str) + Send + Sync + Clone + 'static,
{
    analyze_with_evidence(app, input, fetched, vec![], on_stage).await
}

pub async fn analyze_with_market_evidence<F>(
    app: &AppHandle,
    input: PublishingAssistantInput,
    fetched: data_fetcher::CapturedInsights,
    competitors: Vec<data_fetcher::CapturedInsights>,
    on_stage: F,
) -> Result<PublishingAssistantOutput>
where
    F: Fn(&str, &str) + Send + Sync + Clone + 'static,
{
    analyze_with_evidence(app, input, fetched, competitors, on_stage).await
}

async fn analyze_with_evidence<F>(
    app: &AppHandle,
    input: PublishingAssistantInput,
    fetched: data_fetcher::CapturedInsights,
    competitors: Vec<data_fetcher::CapturedInsights>,
    on_stage: F,
) -> Result<PublishingAssistantOutput>
where
    F: Fn(&str, &str) + Send + Sync + Clone + 'static,
{
    if input.media_note.trim().is_empty() {
        return Err(anyhow!("اقرأ الوسائط بالنموذج البصري ثم وافق على الوصف قبل تشغيل Hermes."));
    }
    if input.client.countries.is_empty() {
        return Err(anyhow!("اختر دولة واحدة على الأقل حتى يعمل الاستهداف الجغرافي."));
    }

    on_stage("assistant", "فلترة النيتش والدول + دمج الإحصائيات الفعلية…");
    let rule = niche_rule(&input.client.niche);
    let selected_countries = selected_countries(&input.client.countries);

    let bundle = insights::gather(app, &input.client, on_stage.clone()).await?;
    let scored = rank_hashtags(&bundle.trending_hashtags, rule, &selected_countries);
    let deterministic_time = best_time(&input.platform, &selected_countries, !bundle.best_times_weekend.is_empty());
    let allowed: Vec<String> = scored.iter().map(|h| h.tag.clone()).collect();

    on_stage("vision", "تحليل الوسائط محلياً عبر Vision Agent…");
    let media_note = enrich_media_note(app, input.media_path.as_deref(), &input.media_note).await;

    on_stage("assistant", "صياغة كابشن المنصات مع البيانات الفعلية…");
    // Build context and append fetched insights block.
    let mut context = build_strategy_context(&input, &bundle, None, &allowed, &deterministic_time, rule);
    context.push_str(&data_fetcher::to_context_block(&fetched));
    context.push_str(&data_fetcher::competitor_context_block(&competitors));

    let ai = llm::generate_publishing_assistant(
        &input.client,
        &input.platform,
        &context,
        &media_note,
        &allowed,
        &deterministic_time,
    )
    .await
    .ok()
    .and_then(|text| parse_ai_copy(&text));

    // Reuse the same assembly logic as `analyze`.
    let instagram_caption = ai
        .as_ref()
        .and_then(|x| x.instagram.as_ref())
        .and_then(|x| x.caption.clone())
        .unwrap_or_else(|| fallback_caption("Instagram", &input, &selected_countries));
    let tiktok_caption = ai
        .as_ref()
        .and_then(|x| x.tiktok.as_ref())
        .and_then(|x| x.caption.clone())
        .unwrap_or_else(|| fallback_caption("TikTok", &input, &selected_countries));
    let visual_note = ai
        .as_ref()
        .and_then(|x| x.visual_note.clone())
        .unwrap_or_else(|| "راجع أول ثانيتين، وضوح المنتج/الفكرة، ووجود دعوة تفاعل مباشرة قبل الاعتماد.".into());
    let summary = ai
        .as_ref()
        .and_then(|x| x.summary.clone())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "استراتيجية {} لـ {} — معززة بإحصائيات فعلية ({} نقطة API).",
                rule.canonical,
                input.client.name,
                fetched.raw_endpoints.len()
            )
        });

    let deterministic_score = success_score(&input, &bundle, &scored, false);
    let score = ai
        .as_ref()
        .and_then(|x| x.success_score)
        .map(|s| s.clamp(1, 100))
        .unwrap_or(deterministic_score);
    let suggested_time = ai
        .as_ref()
        .and_then(|x| x.suggested_time.clone())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| deterministic_time.clone());
    let instagram_tags = sanitize_model_tags(
        ai.as_ref().and_then(|x| x.instagram.as_ref()).and_then(|x| x.hashtags.clone()),
        &allowed, 16,
    );
    let tiktok_tags = sanitize_model_tags(
        ai.as_ref().and_then(|x| x.tiktok.as_ref()).and_then(|x| x.hashtags.clone()),
        &allowed, 10,
    );
    let rationale = ai
        .as_ref()
        .and_then(|x| x.strategy_insights.as_ref().map(|i| vec![i.reasoning.clone()]))
        .filter(|v| !v.is_empty())
        .or_else(|| ai.as_ref().and_then(|x| x.rationale.clone()).filter(|v| !v.is_empty()))
        .unwrap_or_else(|| default_rationale(rule, &selected_countries, false));
    let fallback_virality = virality_indicators(score, &input.media_note, &scored);
    let virality_indicators = ai
        .as_ref()
        .and_then(|x| x.virality_indicators.clone())
        .map(|v| normalize_virality_indicators(v, &fallback_virality))
        .unwrap_or_else(|| fallback_virality.clone());
    let selected_platform_card = ai.as_ref().and_then(|x| {
        exact_model_card(x, &input, rule, &selected_countries, &allowed, score, &suggested_time, &virality_indicators)
    });
    let geo_profile = geo_profile(&selected_countries);
    let instagram_preview = PlatformPreview {
        platform: "Instagram".into(),
        caption: instagram_caption,
        hashtags: instagram_tags,
        suggested_time: suggested_time.clone(),
        visual_note: visual_note.clone(),
    };
    let tiktok_preview = PlatformPreview {
        platform: "TikTok".into(),
        caption: tiktok_caption,
        hashtags: tiktok_tags,
        suggested_time: suggested_time.clone(),
        visual_note,
    };
    let cards = build_cards(
        &input, rule, &selected_countries, &scored,
        &instagram_preview, &tiktok_preview, score, &suggested_time,
        &virality_indicators, selected_platform_card, &rationale,
    );

    Ok(PublishingAssistantOutput {
        summary,
        success_score: score,
        suggested_time: suggested_time.clone(),
        timezone: geo_profile.timezone.clone(),
        geo_profile,
        hashtags: scored.clone(),
        instagram: instagram_preview,
        tiktok: tiktok_preview,
        virality_indicators,
        cards,
        rationale,
        data_points: {
            let mut pts = data_points(&bundle, None);
            pts.push(format!("إحصائيات فعلية محتجزة: {} نقطة API", fetched.raw_endpoints.len()));
            if let Some(er) = fetched.normalized.engagement_rate {
                pts.push(format!("معدل التفاعل الفعلي: {:.2}%", er));
            }
            if !competitors.is_empty() {
                pts.push(format!(
                    "منشورات منافسة محللة: {}، وتم تمرير أفضل 5 حسب التفاعل إلى Hermes",
                    competitors.len()
                ));
            }
            pts
        },
        guardrails: vec![
            "لا يتم اقتراح هاشتاق خارج تقاطع النيتش والدول المحددة.".into(),
            "التحليل يعمل محلياً، وقرار النشر النهائي يبقى بيد المستخدم.".into(),
            "الإحصائيات الفعلية محلية من الجلسة ولا تُرسَل لأي خدمة خارجية.".into(),
        ],
    })
}
