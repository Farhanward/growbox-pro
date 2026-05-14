#!/usr/bin/env node
/**
 * GrowBox Pro — Data Fetching Module
 * Connects to the isolated browser (already running with --remote-debugging-port)
 * via Playwright CDP, intercepts raw JSON API responses from Instagram/TikTok,
 * normalizes the three virality indicators, and writes results to stdout.
 *
 * Usage: node playwright-fetcher.mjs <platform> <post_url> <debug_port>
 * Output: NDJSON — lines are either progress objects or the final result object.
 */

import { chromium } from 'playwright-core';

const [, , platform, postUrl, debugPortArg] = process.argv;
const debugPort = parseInt(debugPortArg ?? '9222', 10);

if (!platform || !postUrl) {
  writeError('الاستخدام: node playwright-fetcher.mjs <platform> <post_url> <debug_port>');
  process.exit(1);
}

// ── API endpoint patterns ───────────────────────────────────────────────────

const INSTAGRAM_PATTERNS = [
  /\/api\/v1\/media\/[^/?]+\/insights\//i,
  /\/api\/v1\/media\/[^/?]+\/info\//i,
  /\/api\/v1\/feed\/user\//i,
  /graph\.instagram\.com.*\/insights/i,
  /graph\.facebook\.com.*\/insights/i,
];

const TIKTOK_PATTERNS = [
  /tiktok\.com\/api\/item\/detail/i,
  /tiktok\.com\/api\/post\/item_list/i,
  /aweme\/v1\/feed/i,
  /aweme\/v1\/aweme\/detail/i,
  /api.*tiktok.*item_detail/i,
];

const patterns = platform === 'tiktok' ? TIKTOK_PATTERNS : INSTAGRAM_PATTERNS;

// ── stdout helpers ──────────────────────────────────────────────────────────

function writeProgress(stage, message) {
  process.stdout.write(
    JSON.stringify({ __progress: true, stage, message }) + '\n',
  );
}

function writeError(message) {
  process.stderr.write(JSON.stringify({ error: message }) + '\n');
}

// ── main ────────────────────────────────────────────────────────────────────

async function main() {
  writeProgress('connect', 'الاتصال بالمتصفح المعزول عبر CDP…');

  let browser;
  try {
    browser = await chromium.connectOverCDP(
      `http://127.0.0.1:${debugPort}`,
      { timeout: 10_000 },
    );
  } catch (e) {
    writeError(
      `فشل الاتصال بالمتصفح على المنفذ ${debugPort}: ${e.message}. ` +
      'تأكد من فتح المتصفح المعزول من GrowBox أولاً.',
    );
    process.exit(1);
  }

  const contexts = browser.contexts();
  if (contexts.length === 0) {
    writeError('لا توجد نوافذ مفتوحة في المتصفح المعزول.');
    await browser.close();
    process.exit(1);
  }

  const context = contexts[0];

  if (postUrl.startsWith('discover:')) {
    const hashtags = postUrl
      .slice('discover:'.length)
      .split(',')
      .map((x) => x.trim().replace(/^#/, ''))
      .filter(Boolean)
      .slice(0, 8);
    const urls = await discoverPostUrls(context, platform, hashtags);
    process.stdout.write(
      JSON.stringify({
        __result: true,
        platform,
        post_url: postUrl,
        captured_at: new Date().toISOString(),
        raw_endpoints: urls.map((url) => ({ url })),
        normalized: {},
        discovered_urls: urls,
      }) + '\n',
    );
    await browser.close();
    return;
  }

  const captured = [];
  const diagnostics = [];

  // Observe JSON responses without modifying the request flow. This is more
  // reliable with TikTok/Instagram than route.fetch(), and keeps the browser
  // behavior identical to a normal user session.
  const pendingCaptures = new Set();
  context.on('response', (response) => {
    const task = captureResponse(response, captured)
      .catch(() => {})
      .finally(() => pendingCaptures.delete(task));
    pendingCaptures.add(task);
  });

  // Navigate to the post URL — this triggers fresh API calls.
  writeProgress('navigate', 'فتح رابط المنشور في المتصفح…');
  let page = context.pages()[0];
  if (!page) page = await context.newPage();

  try {
    await page.goto(postUrl, { waitUntil: 'networkidle', timeout: 35_000 });
  } catch {
    // networkidle timeout is acceptable — we may already have what we need.
  }

  // Extra wait for deferred/lazy API calls (e.g. Insights loaded after scroll).
  writeProgress('wait', 'انتظار تحميل البيانات الكاملة…');
  await page.waitForTimeout(4_000);
  await Promise.allSettled([...pendingCaptures]);

  if (captured.length === 0 && platform === 'tiktok') {
    await captureTiktokItemDetail(page, captured, diagnostics);
  }
  if (captured.length === 0 && platform === 'tiktok') {
    await captureTiktokEmbeddedJson(page, captured, diagnostics);
  }
  if (captured.length === 0 && platform === 'instagram') {
    await captureInstagramEmbeddedJson(page, captured, diagnostics);
  }

  if (captured.length === 0) {
    writeError(
      'لم يتم التقاط أي استجابات API. ' +
      'تأكد أنك سجّلت الدخول في المتصفح المعزول وأن الرابط صحيح.' +
      (diagnostics.length ? ` التشخيص: ${diagnostics.join(' ')}` : ''),
    );
    await browser.close();
    process.exit(1);
  }

  writeProgress('normalize', 'تحليل وتطبيع البيانات…');
  const normalized =
    platform === 'tiktok'
      ? normalizeTiktok(captured)
      : normalizeInstagram(captured);

  // Write the final result line.
  process.stdout.write(
    JSON.stringify({
      __result: true,
      platform,
      post_url: postUrl,
      captured_at: new Date().toISOString(),
      raw_endpoints: captured.map((c) => ({ url: c.url })),
      normalized,
    }) + '\n',
  );

  await browser.close();
}

async function captureInstagramEmbeddedJson(page, captured, diagnostics) {
  writeProgress('capture', 'قراءة JSON المضمّن في صفحة Instagram…');
  const shortcode = page.url().match(/instagram\.com\/(?:p|reel|tv)\/([^/?#]+)/i)?.[1] ?? null;
  const result = await page.evaluate((targetShortcode) => {
    for (const script of [...document.querySelectorAll('script[type="application/json"]')]) {
      let data;
      try {
        data = JSON.parse(script.textContent || '{}');
      } catch {
        continue;
      }
      const item = findInstagramItem(data, targetShortcode);
      if (item) return { data: { items: [item] } };
    }
    function findInstagramItem(root, shortcode) {
      const seen = new WeakSet();
      const stack = [root];
      while (stack.length) {
        const obj = stack.pop();
        if (!obj || typeof obj !== 'object' || seen.has(obj)) continue;
        seen.add(obj);
        const isTarget =
          (!shortcode || obj.code === shortcode || obj.shortcode === shortcode) &&
          (obj.like_count != null || obj.comment_count != null || obj.view_count != null || obj.play_count != null);
        if (isTarget) return obj;
        for (const value of Object.values(obj)) {
          if (value && typeof value === 'object') stack.push(value);
        }
      }
      return null;
    }
    return null;
  }, shortcode);
  if (!result) {
    diagnostics.push('لم يتم العثور على JSON منشور مضمّن داخل صفحة Instagram.');
    return;
  }
  captured.push({ url: 'embedded:instagram-application-json', data: result.data });
}

async function captureTiktokEmbeddedJson(page, captured, diagnostics) {
  writeProgress('capture', 'قراءة JSON المضمّن في صفحة TikTok…');
  const result = await page.evaluate(() => {
    const ids = ['SIGI_STATE', '__UNIVERSAL_DATA_FOR_REHYDRATION__', '__NEXT_DATA__'];
    for (const id of ids) {
      const text = document.getElementById(id)?.textContent ?? '';
      if (!text.trim()) continue;
      try {
        const data = JSON.parse(text);
        const item =
          data?.ItemModule ? Object.values(data.ItemModule)[0] :
          data?.__DEFAULT_SCOPE__?.['webapp.video-detail']?.itemInfo?.itemStruct ??
          data?.props?.pageProps?.itemInfo?.itemStruct ??
          null;
        if (item) {
          return { id, data: { itemInfo: { itemStruct: item } } };
        }
      } catch {
        // Try the next structured data block.
      }
    }
    return null;
  });
  if (!result) {
    diagnostics.push('لم يتم العثور على JSON فيديو مضمّن داخل صفحة TikTok.');
    return;
  }
  captured.push({ url: `embedded:${result.id}`, data: result.data });
}

async function captureTiktokItemDetail(page, captured, diagnostics) {
  const finalUrl = page.url();
  const itemId = extractTiktokItemId(finalUrl);
  if (!itemId) {
    diagnostics.push(`تعذر استخراج رقم الفيديو من الرابط النهائي: ${finalUrl}`);
    return;
  }
  writeProgress('capture', 'طلب JSON تفاصيل فيديو TikTok من نفس الجلسة…');
  const apiUrl = new URL('/api/item/detail/', 'https://www.tiktok.com');
  apiUrl.searchParams.set('itemId', itemId);
  apiUrl.searchParams.set('aid', '1988');
  const result = await page.evaluate(async (url) => {
    const response = await fetch(url, {
      credentials: 'include',
      headers: { accept: 'application/json, text/plain, */*' },
    });
    return { status: response.status, contentType: response.headers.get('content-type') ?? '', text: await response.text() };
  }, apiUrl.toString());
  const text = result.text ?? '';
  const json = parseJsonPayload(text);
  if (!json) {
    diagnostics.push(`TikTok item/detail لم يرجع JSON صالحاً (status ${result.status}, ${result.contentType || 'no content-type'}).`);
    return;
  }
  captured.push({ url: apiUrl.toString(), data: json });
}

function extractTiktokItemId(url) {
  return url.match(/\/video\/(\d+)/)?.[1] ?? null;
}

async function captureResponse(response, captured) {
  const url = response.url();
  const matched = patterns.some((p) => p.test(url));
  if (!matched) return;
  const contentType = response.headers()['content-type'] ?? '';
  if (!/json|javascript|text/i.test(contentType)) return;
  const text = await response.text();
  const json = parseJsonPayload(text);
  if (!json) return;
  captured.push({ url, data: json });
  writeProgress('capture', `التقاط: ${url.slice(0, 90)}`);
}

function parseJsonPayload(text) {
  try {
    return JSON.parse(text);
  } catch {
    const start = text.indexOf('{');
    const end = text.lastIndexOf('}');
    if (start < 0 || end <= start) return null;
    try {
      return JSON.parse(text.slice(start, end + 1));
    } catch {
      return null;
    }
  }
}

// ── normalization ────────────────────────────────────────────────────────────

function normalizeInstagram(captured) {
  let reach, impressions, likes, comments, shares, saves, profile_visits;

  for (const { data } of captured) {
    // Insights endpoint — data array with named metrics
    if (Array.isArray(data?.data)) {
      for (const metric of data.data) {
        const val =
          metric?.total_value?.value ??
          metric?.values?.[0]?.value ??
          metric?.value;
        if (val == null) continue;
        switch (metric.name) {
          case 'reach':          reach          = reach          ?? val; break;
          case 'impressions':    impressions    = impressions    ?? val; break;
          case 'likes':          likes          = likes          ?? val; break;
          case 'comments':       comments       = comments       ?? val; break;
          case 'shares':         shares         = shares         ?? val; break;
          case 'saved':          saves          = saves          ?? val; break;
          case 'profile_visits': profile_visits = profile_visits ?? val; break;
        }
      }
    }
    // Feed / media info endpoint
    const item = data?.items?.[0] ?? data?.item;
    if (item) {
      likes          = likes          ?? item.like_count;
      comments       = comments       ?? item.comment_count;
      saves          = saves          ?? item.save_count;
      reach          = reach          ?? item.view_count ?? item.play_count;
      profile_visits = profile_visits ?? item.profile_visits_count;
    }
  }

  const engagement_rate =
    reach != null && likes != null
      ? round2(
          (((likes ?? 0) + (comments ?? 0) + (shares ?? 0)) / reach) * 100,
        )
      : null;

  return { reach, impressions, likes, comments, shares, saves, profile_visits, engagement_rate };
}

function normalizeTiktok(captured) {
  let views, likes, comments, shares, saves;

  for (const { data } of captured) {
    const item =
      data?.itemInfo?.itemStruct ??
      data?.aweme_list?.[0] ??
      data?.aweme_detail ??
      data?.item;
    if (!item) continue;
    const stats = item.stats ?? item.statistics;
    if (!stats) continue;
    views    = views    ?? stats.playCount    ?? stats.play_count;
    likes    = likes    ?? stats.diggCount    ?? stats.digg_count;
    comments = comments ?? stats.commentCount ?? stats.comment_count;
    shares   = shares   ?? stats.shareCount   ?? stats.share_count;
    saves    = saves    ?? stats.collectCount ?? stats.collect_count;
  }

  const engagement_rate =
    views != null && likes != null
      ? round2(
          ((toNumber(likes) + toNumber(comments) + toNumber(shares) + toNumber(saves)) / views) * 100,
        )
      : null;

  return {
    views: toOptionalNumber(views),
    likes: toOptionalNumber(likes),
    comments: toOptionalNumber(comments),
    shares: toOptionalNumber(shares),
    saves: toOptionalNumber(saves),
    engagement_rate,
  };
}

function round2(n) {
  return Math.round(n * 100) / 100;
}

function toNumber(value) {
  const n = Number(value ?? 0);
  return Number.isFinite(n) ? n : 0;
}

function toOptionalNumber(value) {
  if (value == null) return undefined;
  const n = Number(value);
  return Number.isFinite(n) ? n : undefined;
}

async function discoverPostUrls(context, platform, hashtags) {
  const page = context.pages()[0] ?? await context.newPage();
  const urls = new Set();
  for (const tag of hashtags) {
    const target =
      platform === 'tiktok'
        ? `https://www.tiktok.com/search/video?q=${encodeURIComponent('#' + tag)}`
        : `https://www.instagram.com/explore/tags/${encodeURIComponent(tag)}/`;
    writeProgress('discover', `استكشاف منشورات الهاشتاق #${tag}`);
    try {
      await page.goto(target, { waitUntil: 'domcontentloaded', timeout: 25_000 });
      await page.waitForTimeout(3500);
      await page.mouse.wheel(0, 1800);
      await page.waitForTimeout(1500);
      const found = await page.evaluate((platformName) => {
        const anchors = [...document.querySelectorAll('a[href]')].map((a) => a.href);
        return anchors.filter((href) => {
          if (platformName === 'tiktok') return /\/video\/\d+/.test(href);
          return /instagram\.com\/(p|reel)\//.test(href);
        });
      }, platform);
      for (const url of found) {
        urls.add(url.split('?')[0]);
        if (urls.size >= 12) return [...urls];
      }
    } catch {
      // continue with next hashtag
    }
  }
  return [...urls];
}

// ── run ──────────────────────────────────────────────────────────────────────

main().catch((e) => {
  writeError(e.message ?? String(e));
  process.exit(1);
});
