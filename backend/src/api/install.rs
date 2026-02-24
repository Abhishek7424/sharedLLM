use axum::{body::Body, extract::State, http::StatusCode, response::Response};
use futures::StreamExt;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio_stream::wrappers::ReceiverStream;

use crate::AppState;

// ─── POST /api/cluster/install-binaries ──────────────────────────────────────

/// Download and install `llama-server` + `llama-rpc-server` from the latest
/// llama.cpp GitHub release into `~/.sharedmem/bin/`.
///
/// Streams NDJSON progress lines:
///   {"status": "Downloading... 42%"}
///   {"status": "Done", "done": true}
///   {"error": "reason", "done": true}   ← on failure
pub async fn install_binaries(State(_state): State<Arc<AppState>>) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(32);

    tokio::spawn(async move {
        if let Err(e) = run_install(tx.clone()).await {
            let msg = format!(
                "{}\n",
                serde_json::json!({ "error": e.to_string(), "done": true })
            );
            let _ = tx.send(msg).await;
        }
    });

    let stream = ReceiverStream::new(rx).map(Ok::<_, std::convert::Infallible>);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-ndjson")
        .header("Cache-Control", "no-cache")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })
}

// ─── Core install logic ───────────────────────────────────────────────────────

async fn run_install(tx: tokio::sync::mpsc::Sender<String>) -> anyhow::Result<()> {
    macro_rules! send {
        ($json:expr) => {
            let _ = tx.send(format!("{}\n", $json)).await;
        };
    }

    // ── 1. Detect platform ───────────────────────────────────────────────────
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Actual asset names from ggml-org/llama.cpp releases (as of b8147+):
    //   llama-bXXXX-bin-macos-arm64.tar.gz
    //   llama-bXXXX-bin-macos-x64.tar.gz
    //   llama-bXXXX-bin-ubuntu-x64.tar.gz
    //   llama-bXXXX-bin-ubuntu-s390x.tar.gz
    //   llama-bXXXX-bin-win-cpu-x64.zip
    //   llama-bXXXX-bin-win-cpu-arm64.zip
    //
    // Returns (keyword_in_asset_name, is_zip)
    let (asset_keyword, is_zip): (&str, bool) = match (os, arch) {
        ("macos", "aarch64") => ("macos-arm64", false),
        ("macos", "x86_64") => ("macos-x64", false),
        ("linux", "x86_64") => ("ubuntu-x64", false),
        ("linux", "aarch64") => ("ubuntu-arm64", false),
        ("windows", "aarch64") => ("win-cpu-arm64", true),
        ("windows", _) => ("win-cpu-x64", true),
        _ => anyhow::bail!("Unsupported platform: {os}/{arch}"),
    };

    let archive_ext = if is_zip { ".zip" } else { ".tar.gz" };

    send!(serde_json::json!({
        "status": format!("Platform detected: {os}/{arch}")
    }));

    // ── 2. Fetch latest release metadata from GitHub ─────────────────────────
    send!(serde_json::json!({
        "status": "Fetching latest llama.cpp release info from GitHub..."
    }));

    let client = reqwest::Client::builder()
        .user_agent("sharedLLM/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let release: serde_json::Value = client
        .get("https://api.github.com/repos/ggml-org/llama.cpp/releases/latest")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("GitHub API request failed: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse GitHub API response: {e}"))?;

    let tag = release["tag_name"].as_str().unwrap_or("unknown");
    send!(serde_json::json!({ "status": format!("Latest release: {tag}") }));

    // ── 3. Find the right asset ──────────────────────────────────────────────
    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No assets found in the release response"))?;

    let asset = assets
        .iter()
        .find(|a| {
            let name = a["name"].as_str().unwrap_or("");
            name.contains(asset_keyword) && name.ends_with(archive_ext)
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No asset found matching '{asset_keyword}' with extension '{archive_ext}'. \
                 Check https://github.com/ggml-org/llama.cpp/releases for available builds."
            )
        })?;

    let asset_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Asset has no download URL"))?;
    let asset_name = asset["name"].as_str().unwrap_or("llama.archive");
    let asset_size = asset["size"].as_u64().unwrap_or(0);

    send!(serde_json::json!({
        "status": format!("Downloading {asset_name}...")
    }));

    // ── 4. Stream-download to a temp file ────────────────────────────────────
    let tmp_path = std::env::temp_dir().join(format!("sharedllm_llama_cpp{archive_ext}"));
    let mut resp = client
        .get(asset_url)
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Download failed: {e}"))?;

    let mut file = tokio::fs::File::create(&tmp_path).await?;
    let mut downloaded: u64 = 0;
    let mut last_reported_pct: u64 = 0;

    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if asset_size > 0 {
            let pct = downloaded * 100 / asset_size;
            // Report every 5%
            if pct / 5 > last_reported_pct / 5 {
                last_reported_pct = pct;
                send!(serde_json::json!({
                    "status": format!("Downloading... {pct}%"),
                    "pct": pct
                }));
            }
        }
    }
    file.flush().await?;
    drop(file);

    send!(serde_json::json!({ "status": "Download complete. Extracting binaries..." }));

    // ── 5. Prepare install directory ─────────────────────────────────────────
    let install_dir = {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| anyhow::anyhow!("Cannot determine HOME directory"))?;
        std::path::PathBuf::from(home)
            .join(".sharedmem")
            .join("bin")
    };
    tokio::fs::create_dir_all(&install_dir).await?;

    // ── 6. Extract target binaries (blocking I/O) ─────────────────────────────
    let binary_ext = if os == "windows" { ".exe" } else { "" };
    let targets = vec![
        format!("llama-server{binary_ext}"),
        format!("llama-rpc-server{binary_ext}"),
    ];

    let tmp_path_b = tmp_path.clone();
    let install_dir_b = install_dir.clone();
    let targets_b = targets.clone();

    if is_zip {
        tokio::task::spawn_blocking(move || {
            extract_zip(&tmp_path_b, &install_dir_b, &targets_b)
        })
        .await??;
    } else {
        tokio::task::spawn_blocking(move || {
            extract_tar_gz(&tmp_path_b, &install_dir_b, &targets_b)
        })
        .await??;
    }

    // ── 7. Cleanup temp file ─────────────────────────────────────────────────
    let _ = tokio::fs::remove_file(&tmp_path).await;

    let install_path = install_dir.display().to_string();
    send!(serde_json::json!({
        "status": format!("Installed to {install_path}. Binaries are ready."),
        "done": true
    }));

    Ok(())
}

// ─── Archive extraction helpers ───────────────────────────────────────────────

fn extract_zip(
    archive_path: &std::path::Path,
    install_dir: &std::path::Path,
    targets: &[String],
) -> anyhow::Result<()> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut found = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let file_name = entry
            .enclosed_name()
            .and_then(|p| p.file_name().map(|n| n.to_os_string()))
            .unwrap_or_default();
        let file_name_str = file_name.to_string_lossy();

        if targets.iter().any(|t| t.as_str() == file_name_str) {
            let dest = install_dir.join(&*file_name_str);
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
            }

            found.push(file_name_str.to_string());
        }
    }

    if found.is_empty() {
        anyhow::bail!(
            "Neither llama-server nor llama-rpc-server found inside the zip archive."
        );
    }
    Ok(())
}

fn extract_tar_gz(
    archive_path: &std::path::Path,
    install_dir: &std::path::Path,
    targets: &[String],
) -> anyhow::Result<()> {
    let file = std::fs::File::open(archive_path)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    let mut found = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if targets.iter().any(|t| t.as_str() == file_name) {
            let dest = install_dir.join(&file_name);
            entry.unpack(&dest)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
            }

            found.push(file_name);
        }
    }

    if found.is_empty() {
        anyhow::bail!(
            "Neither llama-server nor llama-rpc-server found inside the tar.gz archive."
        );
    }
    Ok(())
}
