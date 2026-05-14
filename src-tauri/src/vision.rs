use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::{AppHandle, Emitter, Manager, Window};
use tokio::io::AsyncWriteExt;

const DEFAULT_MODEL: &str = "qwen2-vl-2b-instruct.gguf";
const DEFAULT_MMPROJ: &str = "mmproj-qwen2-vl-2b-instruct.gguf";
const DEFAULT_MODEL_URL: &str = "https://huggingface.co/ggml-org/Qwen2-VL-2B-Instruct-GGUF/resolve/main/Qwen2-VL-2B-Instruct-Q4_K_M.gguf?download=true";
const DEFAULT_MMPROJ_URL: &str = "https://huggingface.co/ggml-org/Qwen2-VL-2B-Instruct-GGUF/resolve/main/mmproj-Qwen2-VL-2B-Instruct-Q8_0.gguf?download=true";
const DEFAULT_FFMPEG_URL_WINDOWS: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
const DEFAULT_FFMPEG_URL_MACOS: &str = "https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip";

#[derive(Debug, Serialize)]
pub struct VisionInspection {
    pub media_kind: String,
    pub description: String,
    pub source_note: String,
    pub warning: Option<String>,
}

fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("GrowBoxPro").join("models");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn tools_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("GrowBoxPro").join("tools");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn llama_dir(app: &AppHandle) -> Result<PathBuf> {
    Ok(app
        .path()
        .resource_dir()
        .context("no resource_dir")?
        .join("resources")
        .join("llama"))
}

fn ffmpeg_bin_path() -> PathBuf {
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    tools_dir().join("ffmpeg").join(name)
}

fn mtmd_bin(app: &AppHandle) -> Result<PathBuf> {
    let name = if cfg!(windows) { "llama-mtmd-cli.exe" } else { "llama-mtmd-cli" };
    Ok(llama_dir(app)?.join(name))
}

fn model_paths() -> (PathBuf, PathBuf) {
    let model = std::env::var("GROWBOX_QWEN2VL_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|_| data_dir().join(DEFAULT_MODEL));
    let mmproj = std::env::var("GROWBOX_QWEN2VL_MMPROJ")
        .map(PathBuf::from)
        .unwrap_or_else(|_| data_dir().join(DEFAULT_MMPROJ));
    (model, mmproj)
}

pub fn status() -> serde_json::Value {
    let (model, mmproj) = model_paths();
    serde_json::json!({
        "model_path": model.to_string_lossy(),
        "mmproj_path": mmproj.to_string_lossy(),
        "ready": model.exists() && mmproj.exists(),
        "model_hint": "اضغط تحميل النموذج البصري مرة واحدة. يمكن للمستخدم المتقدم وضع ملفات GGUF محلياً من إعدادات النظام.",
        "model_size": file_size(&model),
        "mmproj_size": file_size(&mmproj),
    })
}

pub async fn download_model(window: Window) -> Result<serde_json::Value> {
    let (model, mmproj) = model_paths();
    let model_url = std::env::var("GROWBOX_QWEN2VL_MODEL_URL").unwrap_or_else(|_| DEFAULT_MODEL_URL.into());
    let mmproj_url = std::env::var("GROWBOX_QWEN2VL_MMPROJ_URL").unwrap_or_else(|_| DEFAULT_MMPROJ_URL.into());
    let client = reqwest::Client::new();

    download_file(&client, &window, "model", "Qwen2-VL", &model_url, &model).await?;
    download_file(&client, &window, "mmproj", "Vision projector", &mmproj_url, &mmproj).await?;

    let _ = window.emit(
        "vision:progress",
        serde_json::json!({
            "stage": "ready",
            "file": "Qwen2-VL",
            "percent": 100.0,
            "downloaded": file_size(&model).unwrap_or(0) + file_size(&mmproj).unwrap_or(0),
            "total": file_size(&model).unwrap_or(0) + file_size(&mmproj).unwrap_or(0),
            "message": "النموذج البصري جاهز للاستخدام المحلي."
        }),
    );

    Ok(status())
}

pub async fn describe_media(app: &AppHandle, media_path: Option<&str>) -> Result<Option<String>> {
    describe_media_inner(app, None, media_path).await.map(|value| value.map(|inspection| inspection.description))
}

pub async fn inspect_media(app: &AppHandle, window: &Window, media_path: Option<&str>) -> Result<VisionInspection> {
    describe_media_inner(app, Some(window), media_path)
        .await?
        .ok_or_else(|| anyhow!("اختر صورة أو فيديو أولاً."))
}

async fn describe_media_inner(
    app: &AppHandle,
    window: Option<&Window>,
    media_path: Option<&str>,
) -> Result<Option<VisionInspection>> {
    let Some(path) = media_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let media = PathBuf::from(path);
    if !media.exists() {
        return Ok(Some(VisionInspection {
            media_kind: "missing".into(),
            description: format!(
                "Vision Agent: الملف غير موجود محلياً، لذلك تم الاعتماد على وصف المستخدم فقط: {}",
                media.display()
            ),
            source_note: "لم يتم تحليل ملف فعلي.".into(),
            warning: Some("الملف غير موجود.".into()),
        }));
    }

    let mut warning = None;
    let mut source_note = "تمت قراءة صورة مباشرة عبر Qwen2-VL المحلي.".to_string();
    emit_inspection_progress(window, "prepare", 5.0, "تجهيز ملف الوسائط للقراءة البصرية.");
    let frames = if is_video(&media) {
        emit_inspection_progress(window, "frames", 12.0, "استخراج 3 إطارات من الفيديو: البداية، المنتصف، والنهاية.");
        match extract_video_frames(app, window, &media).await {
            Ok(frames) => {
                source_note = "تمت قراءة الفيديو عبر استخراج 3 إطارات: البداية، المنتصف، والنهاية، ثم تحليلها بـ Qwen2-VL.".into();
                frames
            }
            Err(e) => {
                return Ok(Some(VisionInspection {
                    media_kind: "video".into(),
                    description: format!(
                        "Vision Agent: تعذر استخراج إطار من الفيديو محلياً ({e})."
                    ),
                    source_note: "لم يتم إرسال وصف فيديو صالح إلى Hermes.".into(),
                    warning: Some(format!("تعذر تحليل الفيديو: {e}")),
                }));
            }
        }
    } else {
        vec![media]
    };

    let (model, mmproj) = model_paths();
    if !model.exists() || !mmproj.exists() {
        return Ok(Some(VisionInspection {
            media_kind: if is_video(Path::new(path)) { "video" } else { "image" }.into(),
            description: "Vision Agent غير جاهز: حمّل Qwen2-VL أولاً من خطوة التجهيز.".into(),
            source_note: "لم يتم إرسال وصف بصري إلى Hermes.".into(),
            warning: Some(format!("ملفات النموذج غير موجودة: {} و {}", model.display(), mmproj.display())),
        }));
    }

    let mut parts = Vec::new();
    for (index, frame) in frames.iter().enumerate() {
        let progress_base = if frames.len() == 1 { 25.0 } else { 25.0 + (index as f64 * 20.0) };
        let label = if frames.len() == 1 {
            "الصورة".to_string()
        } else {
            match index {
                0 => "إطار بداية الفيديو".to_string(),
                1 => "إطار منتصف الفيديو".to_string(),
                _ => "إطار نهاية الفيديو".to_string(),
            }
        };
        emit_inspection_progress(
            window,
            "vision",
            progress_base,
            &format!("Qwen2-VL يقرأ {label}."),
        );
        let text = run_qwen2vl(app, &model, &mmproj, frame).await?;
        parts.push(format!("### {}\n{}", label, text.trim()));
        emit_inspection_progress(
            window,
            "vision",
            (progress_base + 18.0).min(92.0),
            &format!("تمت قراءة {label}."),
        );
    }
    emit_inspection_progress(window, "compose", 96.0, "تجميع وصف الوسائط النهائي للمراجعة.");
    let description = format!("## وصف Vision Agent للوسائط\n{}\n", parts.join("\n\n"));
    if frames.len() > 1 {
        for frame in &frames {
            let _ = tokio::fs::remove_file(frame).await;
        }
    }
    if description.trim().is_empty() {
        warning = Some("أعاد النموذج البصري وصفاً فارغاً.".into());
    }
    Ok(Some(VisionInspection {
        media_kind: if is_video(Path::new(path)) { "video" } else { "image" }.into(),
        description,
        source_note,
        warning,
    }))
}

fn emit_inspection_progress(window: Option<&Window>, stage: &str, percent: f64, message: &str) {
    if let Some(window) = window {
        let _ = window.emit(
            "vision:progress",
            serde_json::json!({
                "stage": stage,
                "file": "media",
                "percent": percent,
                "message": message
            }),
        );
    }
}

async fn download_file(
    client: &reqwest::Client,
    window: &Window,
    stage: &str,
    label: &str,
    url: &str,
    dest: &Path,
) -> Result<()> {
    if dest.exists() && file_size(dest).unwrap_or(0) > 0 {
        let size = file_size(dest).unwrap_or(0);
        let _ = window.emit(
            "vision:progress",
            serde_json::json!({
                "stage": stage,
                "file": label,
                "percent": 100.0,
                "downloaded": size,
                "total": size,
                "message": format!("{label} موجود مسبقاً.")
            }),
        );
        return Ok(());
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let tmp = dest.with_extension("download");
    if tmp.exists() {
        let _ = tokio::fs::remove_file(&tmp).await;
    }

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("تعذر الاتصال لتحميل {label}"))?
        .error_for_status()
        .with_context(|| format!("رفض الخادم تحميل {label}"))?;
    let total = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut downloaded = 0_u64;

    let _ = window.emit(
        "vision:progress",
        serde_json::json!({
            "stage": stage,
            "file": label,
            "percent": 0.0,
            "downloaded": 0,
            "total": total,
            "message": format!("بدء تحميل {label}...")
        }),
    );

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("انقطع تحميل {label}"))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        let percent = if total > 0 { (downloaded as f64 / total as f64) * 100.0 } else { 0.0 };
        let _ = window.emit(
            "vision:progress",
            serde_json::json!({
                "stage": stage,
                "file": label,
                "percent": percent,
                "downloaded": downloaded,
                "total": total,
                "message": format!("تحميل {label}...")
            }),
        );
    }

    file.flush().await?;
    drop(file);
    if dest.exists() {
        tokio::fs::remove_file(dest).await?;
    }
    tokio::fs::rename(&tmp, dest).await?;
    Ok(())
}

fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|meta| meta.len())
}

async fn run_qwen2vl(app: &AppHandle, model: &Path, mmproj: &Path, image: &Path) -> Result<String> {
    let bin = mtmd_bin(app)?;
    if !bin.exists() {
        return Err(anyhow!("llama-mtmd-cli غير موجود في {}", bin.display()));
    }
    let lib_dir = llama_dir(app)?;
    let prompt = "Describe this media for a Gulf social-media strategist. Return Arabic bullet points covering: visible subject, scene, colors, text/OCR, product cues, hook strength, first-frame clarity, and risks before publishing.";

    let mut cmd = tokio::process::Command::new(bin);
    cmd.arg("-m")
        .arg(model)
        .arg("--mmproj")
        .arg(mmproj)
        .arg("--image")
        .arg(image)
        .arg("-p")
        .arg(prompt)
        .arg("-n")
        .arg("360")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        let path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{};{}", lib_dir.display(), path));
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().await?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("فشل Vision Agent: {}", err.trim()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(format!("## وصف Vision Agent للوسائط\n{}\n", clean_mtmd_output(&text)))
}

fn clean_mtmd_output(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("load_backend:")
                && !trimmed.starts_with("main:")
                && !trimmed.starts_with("llama_")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn is_video(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()).as_deref(),
        Some("mp4" | "mov" | "m4v" | "webm" | "mkv")
    )
}

async fn extract_video_frames(app: &AppHandle, window: Option<&Window>, path: &Path) -> Result<Vec<PathBuf>> {
    let ffmpeg = resolve_ffmpeg(app, window).await?;
    let duration = video_duration_secs(&ffmpeg, path).await.unwrap_or(6.0).max(3.0);
    let positions = [1.0, (duration / 2.0).max(1.0), (duration - 1.0).max(1.0)];
    let mut frames = Vec::new();
    for seconds in positions {
        let out = std::env::temp_dir().join(format!("growbox-vision-{}.jpg", uuid::Uuid::new_v4().simple()));
        let mut cmd = tokio::process::Command::new(&ffmpeg);
        cmd.arg("-y")
            .arg("-ss")
            .arg(format!("{seconds:.2}"))
            .arg("-i")
            .arg(path)
            .arg("-frames:v")
            .arg("1")
            .arg(&out)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_no_window(&mut cmd);
        let status = cmd.status().await.context("ffmpeg غير مثبت أو غير متاح في PATH")?;
        if status.success() && out.exists() {
            frames.push(out);
        }
    }
    if frames.is_empty() {
        return Err(anyhow!("لم ينتج ffmpeg أي إطار صالح"));
    }
    Ok(frames)
}

async fn video_duration_secs(ffmpeg: &Path, path: &Path) -> Result<f64> {
    let mut cmd = tokio::process::Command::new(ffmpeg);
    cmd.arg("-i")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    configure_no_window(&mut cmd);
    let output = cmd.output().await?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let Some(caps) = stderr.lines().find(|line| line.contains("Duration:")) else {
        return Err(anyhow!("تعذر قراءة مدة الفيديو"));
    };
    let time = caps
        .split("Duration:")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .map(str::trim)
        .ok_or_else(|| anyhow!("تعذر تحليل مدة الفيديو"))?;
    let mut parts = time.split(':');
    let h: f64 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
    let m: f64 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
    let s: f64 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
    Ok((h * 3600.0) + (m * 60.0) + s)
}

async fn resolve_ffmpeg(app: &AppHandle, window: Option<&Window>) -> Result<PathBuf> {
    let ffmpeg_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let bundled = llama_dir(app).ok().map(|dir| dir.join(ffmpeg_name));
    if let Some(path) = bundled.filter(|path| path.exists()) {
        return Ok(path);
    }

    let local = ffmpeg_bin_path();
    if local.exists() {
        return Ok(local);
    }

    if let Ok(path) = which_on_path("ffmpeg") {
        return Ok(path);
    }

    if let Some(window) = window {
        download_ffmpeg(window).await?;
        let local = ffmpeg_bin_path();
        if local.exists() {
            return Ok(local);
        }
    }

    Err(anyhow!("ffmpeg غير مثبت أو غير متاح لاستخراج إطار الفيديو"))
}

async fn download_ffmpeg(window: &Window) -> Result<()> {
    let archive = tools_dir().join("ffmpeg-release.zip");
    let default_url = if cfg!(windows) {
        DEFAULT_FFMPEG_URL_WINDOWS
    } else {
        DEFAULT_FFMPEG_URL_MACOS
    };
    let url = std::env::var("GROWBOX_FFMPEG_URL").unwrap_or_else(|_| default_url.into());
    let client = reqwest::Client::new();
    download_file(&client, window, "ffmpeg", "FFmpeg video reader", &url, &archive).await?;

    let dest = ffmpeg_bin_path();
    let archive_for_task = archive.clone();
    tokio::task::spawn_blocking(move || extract_ffmpeg_from_zip(&archive_for_task, &dest))
        .await
        .context("تعذر تجهيز أداة قراءة الفيديو")??;
    Ok(())
}

fn extract_ffmpeg_from_zip(archive_path: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Windows zip: ffmpeg-*-essentials_build/bin/ffmpeg.exe
    // macOS zip (evermeet.cx): ffmpeg at root
    let mut ffmpeg_index = None;
    for i in 0..archive.len() {
        let name = archive.by_index(i)?.name().replace('\\', "/");
        let is_windows_path = name.ends_with("/bin/ffmpeg.exe");
        let is_macos_path = name == "ffmpeg" || name.ends_with("/ffmpeg");
        if is_windows_path || is_macos_path {
            ffmpeg_index = Some(i);
            break;
        }
    }
    let index = ffmpeg_index.ok_or_else(|| anyhow!("لم يتم العثور على ffmpeg داخل الحزمة"))?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut entry = archive.by_index(index)?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    let mut out = std::fs::File::create(dest)?;
    out.write_all(&bytes)?;
    // On Unix, set the executable bit.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn which_on_path(name: &str) -> Result<PathBuf> {
    let path = std::env::var_os("PATH").ok_or_else(|| anyhow!("PATH غير متاح"))?;
    for dir in std::env::split_paths(&path) {
        let candidate = if cfg!(windows) { dir.join(format!("{name}.exe")) } else { dir.join(name) };
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(anyhow!("{name} غير موجود في PATH"))
}

fn configure_no_window(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}
