# Reach Optimizer

تطبيق macOS أصلي يولّد خطط انتشار للمصورين والمبدعين الخليجيين على TikTok / Instagram / Snapchat — **بدون تسجيل دخول لأي حساب**.

## للمستخدم النهائي

1. حمّل `ReachOptimizer-1.0.dmg`
2. افتحه واسحب الأيقونة إلى مجلد **Applications**
3. **أول مرة فقط**: كليك يمين على الأيقونة → **Open** → **Open** (لتجاوز تحذير "تطبيق غير معروف")
4. أول تشغيل سيُحمَّل النموذج (~5.8 GB) — انتظر شريط التقدّم
5. ادخل اسم العميل، الحسابات العامة، وولّد التقرير

**لإلغاء التثبيت**: اسحب الأيقونة من Applications إلى Trash.

## للمطوّر

```bash
npm install
npm run tauri dev          # وضع التطوير (Windows/Mac/Linux)
npm run tauri build        # حزمة محلية
```

## البناء للتوزيع (macOS DMG)

ادفع تاج بصيغة `vX.Y.Z` ليقوم GitHub Actions ببناء `.dmg` تلقائياً على Mac runner:

```bash
git tag v0.1.0 && git push --tags
```

## البنية

- **Backend**: Rust + Tauri 2 + SQLite + reqwest
- **Frontend**: SvelteKit + Tailwind (RTL)
- **LLM**: AceGPT-v2-8B (Q5_K_M) عبر llama.cpp (Metal)
- **بيانات**: عامة فقط — صفر OAuth، صفر بوتات

## الترخيص

MIT
