use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;

use crate::AppState;

/// GET /agent/install
///
/// Returns an OS-specific shell script that installs and starts llama-rpc-server.
/// Query param: ?os=linux|macos|windows (defaults to linux)
pub async fn install_script(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let os = params
        .get("os")
        .map(|s| s.as_str())
        .unwrap_or("linux");

    // Detect the host's local IP for display purposes
    let host_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "YOUR_HOST_IP".to_string());

    let rpc_port = state.llama_cpp.rpc_port;
    let dashboard_port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    let (script, content_type) = match os {
        "macos" => (
            macos_script(&host_ip, dashboard_port.as_str(), rpc_port),
            "application/x-sh",
        ),
        "windows" => (
            windows_script(&host_ip, dashboard_port.as_str(), rpc_port),
            "text/plain",
        ),
        _ => (
            linux_script(&host_ip, dashboard_port.as_str(), rpc_port),
            "application/x-sh",
        ),
    };

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, content_type)],
        script,
    )
}

/// GET /agent/info
///
/// Returns JSON info for the Agent page UI.
pub async fn agent_info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let host_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "YOUR_HOST_IP".to_string());

    let dashboard_port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let rpc_port = state.llama_cpp.rpc_port;

    let linux_cmd = format!(
        r#"curl -fsSL "http://{}:{}/agent/install?os=linux" | bash"#,
        host_ip, dashboard_port
    );
    let macos_cmd = format!(
        r#"curl -fsSL "http://{}:{}/agent/install?os=macos" | bash"#,
        host_ip, dashboard_port
    );
    let windows_cmd = format!(
        "irm \"http://{}:{}/agent/install?os=windows\" | iex",
        host_ip, dashboard_port
    );

    Json(serde_json::json!({
        "host_ip": host_ip,
        "dashboard_port": dashboard_port,
        "rpc_port": rpc_port,
        "install_commands": {
            "linux": linux_cmd,
            "macos": macos_cmd,
            "windows": windows_cmd,
        },
        "rpc_server_bin_available": crate::llama_cpp::LlamaCppManager::find_rpc_server_bin().is_some(),
    }))
}

// ─── Script templates ─────────────────────────────────────────────────────────

fn linux_script(host_ip: &str, dashboard_port: &str, rpc_port: u16) -> String {
    format!(
        r#"#!/usr/bin/env bash
# SharedLLM RPC Agent Installer - Linux
# This script installs llama-rpc-server and starts it as an agent for the SharedLLM cluster.
# The host at {host_ip}:{dashboard_port} will distribute model layers to this machine.

set -euo pipefail

INSTALL_DIR="$HOME/.sharedmem/bin"
RPC_PORT={rpc_port}

echo "[SharedLLM] Installing RPC agent..."

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)  LLAMA_ARCH="x64" ;;
  aarch64) LLAMA_ARCH="arm64" ;;
  *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Get latest llama.cpp release (repo moved to ggml-org)
echo "[SharedLLM] Fetching latest llama.cpp release info..."
LATEST_TAG=$(curl -fsSL https://api.github.com/repos/ggml-org/llama.cpp/releases/latest | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

DOWNLOAD_URL="https://github.com/ggml-org/llama.cpp/releases/download/$LATEST_TAG/llama-$LATEST_TAG-bin-ubuntu-$LLAMA_ARCH.zip"

mkdir -p "$INSTALL_DIR"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "[SharedLLM] Downloading llama.cpp $LATEST_TAG..."
curl -fsSL -o "$TMPDIR/llama.zip" "$DOWNLOAD_URL" || {{
  echo "[SharedLLM] Download failed. Please install llama.cpp manually."
  echo "  https://github.com/ggml-org/llama.cpp/releases"
  exit 1
}}

cd "$TMPDIR"
unzip -q llama.zip

# Binary may be named 'rpc-server' in recent releases or 'llama-rpc-server' in older ones
RPC_BIN=$(find . -name "rpc-server" -o -name "llama-rpc-server" 2>/dev/null | head -1)
if [ -z "$RPC_BIN" ]; then
  echo "[SharedLLM] Could not find rpc-server binary in archive."
  exit 1
fi
cp "$RPC_BIN" "$INSTALL_DIR/llama-rpc-server"
chmod +x "$INSTALL_DIR/llama-rpc-server"

mkdir -p "$HOME/.sharedmem"
echo "[SharedLLM] Starting llama-rpc-server on port $RPC_PORT..."
nohup "$INSTALL_DIR/llama-rpc-server" --host 0.0.0.0 --port "$RPC_PORT" > "$HOME/.sharedmem/rpc-server.log" 2>&1 &
echo $! > "$HOME/.sharedmem/rpc-server.pid"

echo ""
echo "[SharedLLM] RPC agent started!"
echo "  Listening: 0.0.0.0:$RPC_PORT"
echo "  Log:       $HOME/.sharedmem/rpc-server.log"
echo "  PID file:  $HOME/.sharedmem/rpc-server.pid"
echo ""

# Self-register with the host dashboard
MY_IP=$(ip route get 8.8.8.8 2>/dev/null | grep -oP 'src \K\S+' || hostname -I 2>/dev/null | awk '{{print $1}}' || echo "")
MY_NAME=$(hostname)
if [ -n "$MY_IP" ]; then
  echo "[SharedLLM] Registering with host at {host_ip}:{dashboard_port}..."
  curl -fsSL -X POST "http://{host_ip}:{dashboard_port}/api/devices" \
    -H "Content-Type: application/json" \
    -d "{{\"name\": \"$MY_NAME\", \"ip\": \"$MY_IP\"}}" \
    -o /dev/null 2>/dev/null \
    && echo "[SharedLLM] Registered! Go to http://{host_ip}:{dashboard_port}/devices to approve this device." \
    || echo "[SharedLLM] Could not auto-register. Add manually at http://{host_ip}:{dashboard_port}/devices (Name=$MY_NAME, IP=$MY_IP)"
else
  echo "[SharedLLM] Could not detect local IP. Add this device manually at http://{host_ip}:{dashboard_port}/devices"
fi
"#,
        host_ip = host_ip,
        dashboard_port = dashboard_port,
        rpc_port = rpc_port,
    )
}

fn macos_script(host_ip: &str, dashboard_port: &str, rpc_port: u16) -> String {
    format!(
        r#"#!/usr/bin/env bash
# SharedLLM RPC Agent Installer - macOS
# Installs llama-rpc-server and starts it as a cluster agent.

set -euo pipefail

INSTALL_DIR="$HOME/.sharedmem/bin"
RPC_PORT={rpc_port}

echo "[SharedLLM] Installing RPC agent for macOS..."

# Prefer Homebrew if available
if command -v brew &>/dev/null; then
  echo "[SharedLLM] Installing llama.cpp via Homebrew..."
  brew install llama.cpp
  LLAMA_RPC=$(which llama-rpc-server 2>/dev/null || echo "")
else
  echo "[SharedLLM] Homebrew not found. Downloading pre-built binary..."
  ARCH=$(uname -m)
  mkdir -p "$INSTALL_DIR"
  LATEST_TAG=$(curl -fsSL https://api.github.com/repos/ggml-org/llama.cpp/releases/latest | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
  DOWNLOAD_URL="https://github.com/ggml-org/llama.cpp/releases/download/$LATEST_TAG/llama-$LATEST_TAG-bin-macos-$ARCH.zip"

  TMPDIR=$(mktemp -d)
  trap 'rm -rf "$TMPDIR"' EXIT
  curl -fsSL -o "$TMPDIR/llama.zip" "$DOWNLOAD_URL" || {{
    echo "Download failed. Install manually: brew install llama.cpp"
    exit 1
  }}
  cd "$TMPDIR" && unzip -q llama.zip
  # Binary may be named 'rpc-server' in recent releases or 'llama-rpc-server' in older ones
  RPC_BIN=$(find . -name "rpc-server" -o -name "llama-rpc-server" 2>/dev/null | head -1)
  if [ -z "$RPC_BIN" ]; then
    echo "[SharedLLM] Could not find rpc-server binary in archive."
    exit 1
  fi
  cp "$RPC_BIN" "$INSTALL_DIR/llama-rpc-server"
  chmod +x "$INSTALL_DIR/llama-rpc-server"
  LLAMA_RPC="$INSTALL_DIR/llama-rpc-server"
fi

mkdir -p "$HOME/.sharedmem"
echo "[SharedLLM] Starting llama-rpc-server on port $RPC_PORT..."
nohup "${{LLAMA_RPC:-llama-rpc-server}}" --host 0.0.0.0 --port "$RPC_PORT" \
  > "$HOME/.sharedmem/rpc-server.log" 2>&1 &
echo $! > "$HOME/.sharedmem/rpc-server.pid"

echo ""
echo "[SharedLLM] RPC agent started!"
echo "  Listening: 0.0.0.0:$RPC_PORT"
echo "  Dashboard: http://{host_ip}:{dashboard_port}"
echo ""

# Self-register with the host dashboard
MY_IP=$(ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || ifconfig 2>/dev/null | grep 'inet ' | grep -v 127.0.0.1 | awk '{{print $2}}' | head -1 || echo "")
MY_NAME=$(hostname)
if [ -n "$MY_IP" ]; then
  echo "[SharedLLM] Registering with host at {host_ip}:{dashboard_port}..."
  curl -fsSL -X POST "http://{host_ip}:{dashboard_port}/api/devices" \
    -H "Content-Type: application/json" \
    -d "{{\"name\": \"$MY_NAME\", \"ip\": \"$MY_IP\"}}" \
    -o /dev/null 2>/dev/null \
    && echo "[SharedLLM] Registered! Go to http://{host_ip}:{dashboard_port}/devices to approve this device." \
    || echo "[SharedLLM] Could not auto-register. Add manually at http://{host_ip}:{dashboard_port}/devices (Name=$MY_NAME, IP=$MY_IP)"
else
  echo "[SharedLLM] Could not detect local IP. Add this device manually at http://{host_ip}:{dashboard_port}/devices"
fi
"#,
        host_ip = host_ip,
        dashboard_port = dashboard_port,
        rpc_port = rpc_port,
    )
}

fn windows_script(host_ip: &str, dashboard_port: &str, rpc_port: u16) -> String {
    format!(
        r#"# SharedLLM RPC Agent Installer - Windows (PowerShell)
# Run with: irm http://{host_ip}:{dashboard_port}/agent/install?os=windows | iex

$InstallDir = "$env:USERPROFILE\.sharedmem\bin"
$RpcPort = {rpc_port}
$LogFile = "$env:USERPROFILE\.sharedmem\rpc-server.log"

Write-Host "[SharedLLM] Installing RPC agent for Windows..."

# Create install directory
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.sharedmem" | Out-Null

# Get latest release (repo moved to ggml-org)
$Release = Invoke-RestMethod "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest"
$Tag = $Release.tag_name

# Try avx2 first, fall back to cpu (older assets used avx2-x64, newer use cpu-x64)
$DownloadUrl = "https://github.com/ggml-org/llama.cpp/releases/download/$Tag/llama-$Tag-bin-win-avx2-x64.zip"
$TmpZip = "$env:TEMP\llama-cpp.zip"

Write-Host "[SharedLLM] Downloading llama.cpp $Tag..."
try {{
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TmpZip -ErrorAction Stop
}} catch {{
    $DownloadUrl = "https://github.com/ggml-org/llama.cpp/releases/download/$Tag/llama-$Tag-bin-win-cpu-x64.zip"
    Write-Host "[SharedLLM] avx2 build not found, trying cpu build..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TmpZip
}}

$TmpDir = "$env:TEMP\llama-cpp-extract"
Expand-Archive -Path $TmpZip -DestinationPath $TmpDir -Force

# Binary may be named 'rpc-server.exe' in recent releases or 'llama-rpc-server.exe' in older ones
$RpcBin = Get-ChildItem -Path $TmpDir -Recurse -Filter "rpc-server.exe" | Select-Object -First 1
if (-not $RpcBin) {{
    $RpcBin = Get-ChildItem -Path $TmpDir -Recurse -Filter "llama-rpc-server.exe" | Select-Object -First 1
}}
if (-not $RpcBin) {{
    Write-Host "[SharedLLM] Could not find rpc-server binary in archive. Aborting."
    exit 1
}}
Copy-Item $RpcBin.FullName "$InstallDir\llama-rpc-server.exe"

Write-Host "[SharedLLM] Starting llama-rpc-server on port $RpcPort..."
Start-Process -FilePath "$InstallDir\llama-rpc-server.exe" `
  -ArgumentList "--host 0.0.0.0 --port $RpcPort" `
  -RedirectStandardOutput $LogFile `
  -WindowStyle Hidden

Write-Host ""
Write-Host "[SharedLLM] RPC agent started!"
Write-Host "  Listening: 0.0.0.0:$RpcPort"
Write-Host "  Dashboard: http://{host_ip}:{dashboard_port}"
Write-Host ""

# Self-register with the host dashboard
$MyIp = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object {{ $_.IPAddress -notmatch '^127' -and $_.IPAddress -notmatch '^169' }} | Select-Object -First 1).IPAddress
$MyName = $env:COMPUTERNAME
if ($MyIp) {{
    Write-Host "[SharedLLM] Registering with host at {host_ip}:{dashboard_port}..."
    try {{
        $Body = '{{\"name\": \"' + $MyName + '\", \"ip\": \"' + $MyIp + '\"}}'
        Invoke-RestMethod -Uri "http://{host_ip}:{dashboard_port}/api/devices" -Method Post -ContentType "application/json" -Body $Body | Out-Null
        Write-Host "[SharedLLM] Registered! Go to http://{host_ip}:{dashboard_port}/devices to approve this device."
    }} catch {{
        Write-Host "[SharedLLM] Could not auto-register. Add manually at http://{host_ip}:{dashboard_port}/devices (Name=$MyName, IP=$MyIp)"
    }}
}} else {{
    Write-Host "[SharedLLM] Could not detect local IP. Add this device manually at http://{host_ip}:{dashboard_port}/devices"
}}
"#,
        host_ip = host_ip,
        dashboard_port = dashboard_port,
        rpc_port = rpc_port,
    )
}
