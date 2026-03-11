use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct PullModelRequest {
    pub name: String,
}

/// GET /api/models
pub async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.ollama.list_models().await {
        Ok(models) => Json(serde_json::json!({ "models": models })).into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/models/pull
/// Streams the Ollama pull response so the client gets progress lines in real time.
pub async fn pull_model(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PullModelRequest>,
) -> impl IntoResponse {
    // Validate model name: only safe chars, max 200 chars (VULN-21)
    let name_ok = !req.name.is_empty()
        && req.name.len() <= 200
        && req.name.chars().all(|c| c.is_ascii_alphanumeric() || ":-./_".contains(c));
    if !name_ok {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::json!({ "error": "Invalid model name" }).to_string(),
            ))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::empty())
                    .unwrap()
            });
    }

    match state.ollama.pull_model_stream(&req.name).await {
        Ok(response) => {
            let status = response.status();
            // Convert the reqwest byte stream into an axum Body so we stream
            // progress NDJSON lines to the client without buffering the whole body.
            let stream = response.bytes_stream();
            Response::builder()
                .status(status)
                .header("Content-Type", "application/x-ndjson")
                .body(Body::from_stream(stream))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                })
        }
        Err(e) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::json!({ "error": e.to_string() }).to_string(),
            ))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::empty())
                    .unwrap()
            }),
    }
}

/// DELETE /api/models/:name
pub async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.ollama.delete_model(&name).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// A local GGUF model file found during scanning.
#[derive(Serialize)]
pub struct LocalGgufModel {
    pub path: String,
    pub name: String,
    pub size_mb: u64,
}

/// GET /api/models/scan
///
/// Walks a set of well-known directories and returns every `.gguf` file found
/// (excluding tiny vocab-only test files < 10 MB).  The scan is capped at
/// 10 000 entries and skips hidden directories to stay fast.
pub async fn scan_local_models() -> impl IntoResponse {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());

    let search_roots: Vec<std::path::PathBuf> = vec![
        // User home subdirs that commonly hold models
        format!("{home}/models").into(),
        format!("{home}/llm").into(),
        format!("{home}/llms").into(),
        format!("{home}/Downloads").into(),
        format!("{home}/.cache/huggingface/hub").into(),
        format!("{home}/.cache/lm-studio/models").into(),
        format!("{home}/Library/Application Support/LM Studio/models").into(),
        format!("{home}/llama.cpp/models").into(),
        format!("{home}/.sharedmem/models").into(),
        // System-wide
        "/opt/models".into(),
        "/usr/local/share/models".into(),
        "/var/lib/llm".into(),
    ];

    let mut found: Vec<LocalGgufModel> = Vec::new();
    let limit = 10_000usize;

    for root in &search_roots {
        if !root.is_dir() {
            continue;
        }
        walk_gguf(root, &mut found, &mut 0, limit);
    }

    // Sort by name for a consistent, friendly order
    found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Json(serde_json::json!({ "models": found })).into_response()
}

/// Recursive walker: collects `.gguf` files that are ≥ 10 MB (skip vocab tests).
fn walk_gguf(
    dir: &std::path::Path,
    out: &mut Vec<LocalGgufModel>,
    count: &mut usize,
    limit: usize,
) {
    if *count >= limit {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if *count >= limit {
            break;
        }
        let path = entry.path();
        // Skip hidden files / directories
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }
        if path.is_dir() {
            walk_gguf(&path, out, count, limit);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("gguf"))
            .unwrap_or(false)
        {
            let size_mb = std::fs::metadata(&path)
                .map(|m| m.len() / (1024 * 1024))
                .unwrap_or(0);
            // Skip tiny vocab-only files (< 50 MB)
            if size_mb < 50 {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            out.push(LocalGgufModel {
                path: path.to_string_lossy().into_owned(),
                name,
                size_mb,
            });
            *count += 1;
        }
    }
}

/// GET /api/ollama/status
pub async fn ollama_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let running = state.ollama.is_healthy().await;
    Json(serde_json::json!({
        "running": running,
        "host": state.ollama.host,
    }))
}

// ─── HuggingFace search + download ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct HfSearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}
fn default_limit() -> u32 { 20 }

#[derive(Serialize, Deserialize)]
pub struct HfModelResult {
    pub id: String,
    pub downloads: Option<u64>,
    pub likes: Option<u64>,
    pub tags: Option<Vec<String>>,
}

/// GET /api/models/hf-search?q=llama&limit=20
///
/// Queries the HuggingFace Hub API for models tagged `gguf` matching the search
/// term.  Results are sorted by downloads (most popular first).
pub async fn hf_search(Query(q): Query<HfSearchQuery>) -> impl IntoResponse {
    // Sanitise: only allow safe search characters
    let safe: String = q.q.chars()
        .filter(|c| c.is_alphanumeric() || " .-_:/".contains(*c))
        .take(200)
        .collect();
    if safe.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Query must not be empty" })),
        ).into_response();
    }

    let limit = q.limit.clamp(1, 50);
    let url = format!(
        "https://huggingface.co/api/models?search={}&filter=gguf&sort=downloads&direction=-1&limit={}",
        urlencoding::encode(&safe),
        limit
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("sharedLLM/1.0")
        .build()
        .unwrap_or_default();

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    // HF returns an array of model objects
                    let models: Vec<HfModelResult> = json
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|m| serde_json::from_value(m.clone()).ok())
                        .collect();
                    Json(serde_json::json!({ "models": models })).into_response()
                }
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": format!("Failed to parse HF response: {e}") })),
                ).into_response(),
            }
        }
        Ok(resp) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": format!("HuggingFace returned HTTP {}", resp.status()) })),
        ).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": format!("Failed to reach HuggingFace: {e}") })),
        ).into_response(),
    }
}

#[derive(Deserialize)]
pub struct HfFilesQuery {
    pub repo: String,
}

#[derive(Serialize)]
pub struct HfGgufFile {
    pub filename: String,
    pub size_bytes: Option<u64>,
    pub download_url: String,
}

/// GET /api/models/hf-files?repo=TheBloke/Llama-2-7B-GGUF
///
/// Returns the list of .gguf files available in the given HF repository.
pub async fn hf_list_files(Query(q): Query<HfFilesQuery>) -> impl IntoResponse {
    // Sanitise repo ID: allow alphanumeric, hyphens, underscores, dots, forward slash
    let safe_repo: String = q.repo.chars()
        .filter(|c| c.is_alphanumeric() || "-_./".contains(*c))
        .take(200)
        .collect();
    if safe_repo.trim().is_empty() || safe_repo.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid repository ID" })),
        ).into_response();
    }

    let url = format!("https://huggingface.co/api/models/{safe_repo}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("sharedLLM/1.0")
        .build()
        .unwrap_or_default();

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let siblings = json["siblings"].as_array().cloned().unwrap_or_default();
                    let files: Vec<HfGgufFile> = siblings
                        .iter()
                        .filter_map(|s| {
                            let filename = s["rfilename"].as_str()?;
                            if !filename.to_ascii_lowercase().ends_with(".gguf") {
                                return None;
                            }
                            let size_bytes = s["size"].as_u64();
                            let download_url = format!(
                                "https://huggingface.co/{safe_repo}/resolve/main/{filename}"
                            );
                            Some(HfGgufFile {
                                filename: filename.to_string(),
                                size_bytes,
                                download_url,
                            })
                        })
                        .collect();
                    Json(serde_json::json!({ "files": files })).into_response()
                }
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": format!("Failed to parse HF response: {e}") })),
                ).into_response(),
            }
        }
        Ok(resp) if resp.status() == 404 => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Repository not found on HuggingFace" })),
        ).into_response(),
        Ok(resp) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": format!("HuggingFace returned HTTP {}", resp.status()) })),
        ).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": format!("Failed to reach HuggingFace: {e}") })),
        ).into_response(),
    }
}

#[derive(Deserialize)]
pub struct HfDownloadRequest {
    /// HuggingFace repo ID, e.g. "TheBloke/Llama-2-7B-GGUF"
    pub repo: String,
    /// Filename within the repo, e.g. "llama-2-7b.Q4_K_M.gguf"
    pub filename: String,
}

/// POST /api/models/hf-download
///
/// Downloads a .gguf file from HuggingFace into `~/.sharedmem/models/<repo>/`.
/// Streams NDJSON progress lines:
///   `{"progress": 45.2, "downloaded_mb": 1800, "total_mb": 4000}`
/// Final line on success:
///   `{"done": true, "path": "/home/user/.sharedmem/models/..."}`
/// On error:
///   `{"error": "..."}`
pub async fn hf_download(Json(req): Json<HfDownloadRequest>) -> impl IntoResponse {
    // Validate repo and filename
    let safe_repo: String = req.repo.chars()
        .filter(|c| c.is_alphanumeric() || "-_./".contains(*c))
        .take(200)
        .collect();
    let safe_filename: String = req.filename.chars()
        .filter(|c| c.is_alphanumeric() || "-_.".contains(*c))
        .take(300)
        .collect();

    if safe_repo.trim().is_empty()
        || safe_repo.contains("..")
        || safe_filename.trim().is_empty()
        || !safe_filename.to_ascii_lowercase().ends_with(".gguf")
    {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/x-ndjson")
            .body(Body::from(
                serde_json::json!({ "error": "Invalid repo or filename" }).to_string() + "\n",
            ))
            .unwrap_or_else(|_| Response::builder().status(400).body(Body::empty()).unwrap());
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    // Use only the last path component of the repo as the directory name to avoid nested dirs
    let repo_dir_name = safe_repo.replace('/', "__");
    let dest_dir = std::path::PathBuf::from(format!("{home}/.sharedmem/models/{repo_dir_name}"));
    let dest_path = dest_dir.join(&safe_filename);

    let stream = async_stream::stream! {
        // Create destination directory
        if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
            let msg = serde_json::json!({ "error": format!("Cannot create directory: {e}") }).to_string() + "\n";
            yield Ok::<_, std::convert::Infallible>(bytes::Bytes::from(msg));
            return;
        }

        let download_url = format!(
            "https://huggingface.co/{safe_repo}/resolve/main/{safe_filename}"
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(7200)) // 2h for large models
            .user_agent("sharedLLM/1.0")
            .build()
            .unwrap_or_default();

        let resp = match client.get(&download_url).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let msg = serde_json::json!({ "error": format!("HuggingFace returned HTTP {}", r.status()) }).to_string() + "\n";
                yield Ok(bytes::Bytes::from(msg));
                return;
            }
            Err(e) => {
                let msg = serde_json::json!({ "error": format!("Download failed: {e}") }).to_string() + "\n";
                yield Ok(bytes::Bytes::from(msg));
                return;
            }
        };

        let total_bytes = resp.content_length().unwrap_or(0);
        let total_mb = total_bytes / (1024 * 1024);

        // Open the destination file for writing
        let mut file = match tokio::fs::File::create(&dest_path).await {
            Ok(f) => f,
            Err(e) => {
                let msg = serde_json::json!({ "error": format!("Cannot create file: {e}") }).to_string() + "\n";
                yield Ok(bytes::Bytes::from(msg));
                return;
            }
        };

        let mut downloaded: u64 = 0;
        let mut last_reported_pct: u8 = 255; // force first report
        let mut stream = resp.bytes_stream();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(data) => {
                    use tokio::io::AsyncWriteExt;
                    if let Err(e) = file.write_all(&data).await {
                        let msg = serde_json::json!({ "error": format!("Write error: {e}") }).to_string() + "\n";
                        yield Ok(bytes::Bytes::from(msg));
                        // Remove partial file
                        let _ = tokio::fs::remove_file(&dest_path).await;
                        return;
                    }
                    downloaded += data.len() as u64;

                    // Emit progress at most every 1%
                    let pct = if total_bytes > 0 {
                        ((downloaded * 100) / total_bytes) as u8
                    } else {
                        0
                    };
                    if pct != last_reported_pct {
                        last_reported_pct = pct;
                        let downloaded_mb = downloaded / (1024 * 1024);
                        let msg = serde_json::json!({
                            "progress": pct,
                            "downloaded_mb": downloaded_mb,
                            "total_mb": total_mb,
                        }).to_string() + "\n";
                        yield Ok(bytes::Bytes::from(msg));
                    }
                }
                Err(e) => {
                    let msg = serde_json::json!({ "error": format!("Stream error: {e}") }).to_string() + "\n";
                    yield Ok(bytes::Bytes::from(msg));
                    let _ = tokio::fs::remove_file(&dest_path).await;
                    return;
                }
            }
        }

        let final_msg = serde_json::json!({
            "done": true,
            "path": dest_path.to_string_lossy(),
        }).to_string() + "\n";
        yield Ok(bytes::Bytes::from(final_msg));
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-ndjson")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| Response::builder().status(500).body(Body::empty()).unwrap())
}
