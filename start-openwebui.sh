#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────
#  Start Open WebUI on port 3000, pre-configured to use the
#  SharedLLM backend (port 8080) as its OpenAI-compatible endpoint.
# ─────────────────────────────────────────────────────────────────

set -e

PYTHON="/opt/homebrew/bin/python3.12"

# Verify Python 3.12 is available
if ! command -v "$PYTHON" &>/dev/null; then
  echo "Error: Python 3.12 not found at $PYTHON"
  echo "Install via: brew install python@3.12"
  exit 1
fi

# Verify open-webui is installed
if ! "$PYTHON" -m open_webui --help &>/dev/null 2>&1; then
  echo "Installing open-webui..."
  "$PYTHON" -m pip install open-webui --break-system-packages
fi

echo "Starting Open WebUI on http://localhost:3000"
echo "  → AI backend : http://localhost:8080/v1 (SharedLLM llama-server proxy)"
echo "  → Auth       : disabled (local-only mode)"
echo ""

# ── Environment ──────────────────────────────────────────────────
export PORT=3000
export HOST="0.0.0.0"

# Point Open WebUI at the SharedLLM Axum proxy (always reachable on 8080).
# The proxy forwards /v1/models and /v1/chat/completions to llama-server.
export OPENAI_API_BASE_URL="http://localhost:8080/v1"
export OPENAI_API_KEY="sk-sharedllm"

# Disable login screen for local use
export WEBUI_AUTH="False"

# Store Open WebUI data alongside the project
export DATA_DIR="$(dirname "$0")/.openwebui-data"
mkdir -p "$DATA_DIR"

# Allow embedding in SharedLLM's iframe
export CORS_ALLOW_ORIGIN="*"

# ── Launch ───────────────────────────────────────────────────────
exec "$PYTHON" -m open_webui serve --host "$HOST" --port "$PORT"
