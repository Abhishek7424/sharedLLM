#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────
#  SharedLLM — start everything with one command
#    • Rust/Axum backend    → http://localhost:8080
#    • Open WebUI (Chat)    → http://localhost:3000
#    • Frontend (built)     → served by backend at :8080
#
#  Usage:  ./start.sh
#  Stop:   Ctrl+C  (kills all child processes cleanly)
# ─────────────────────────────────────────────────────────────────

set -e

ROOT="$(cd "$(dirname "$0")" && pwd)"
BACKEND="$ROOT/backend"
FRONTEND="$ROOT/frontend"
PYTHON="/opt/homebrew/bin/python3.12"

# ── PIDs to clean up on exit ──────────────────────────────────────
BACKEND_PID=""
OPENWEBUI_PID=""

cleanup() {
  echo ""
  echo "Shutting down all services..."
  [ -n "$BACKEND_PID" ]   && kill "$BACKEND_PID"   2>/dev/null || true
  [ -n "$OPENWEBUI_PID" ] && kill "$OPENWEBUI_PID" 2>/dev/null || true
  wait 2>/dev/null || true
  echo "Done."
}
trap cleanup INT TERM EXIT

# ── Dependency checks ─────────────────────────────────────────────
echo "=== SharedLLM ==="
echo ""

command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found. Install from https://rustup.rs"; exit 1; }
command -v node  >/dev/null 2>&1 || { echo "ERROR: node not found. Install from https://nodejs.org";  exit 1; }

SKIP_OPENWEBUI=""
if [ ! -f "$PYTHON" ]; then
  echo "WARN: Python 3.12 not found at $PYTHON — Chat/Open WebUI will be skipped."
  echo "      Fix with: brew install python@3.12"
  SKIP_OPENWEBUI=1
fi

# ── Install frontend deps if missing ──────────────────────────────
if [ ! -d "$FRONTEND/node_modules" ]; then
  echo "[1/3] Installing frontend dependencies..."
  (cd "$FRONTEND" && npm install --silent)
fi

# ── Build frontend ────────────────────────────────────────────────
echo "[1/3] Building frontend..."
(cd "$FRONTEND" && npm run build --silent)

# ── Build backend binary ──────────────────────────────────────────
echo "[2/3] Building backend..."
(cd "$BACKEND" && cargo build --release 2>&1)

# ── Ensure data directory ─────────────────────────────────────────
mkdir -p "$BACKEND/data"

# ── Start backend in background ───────────────────────────────────
echo "[3/3] Starting services..."
echo ""

(
  cd "$BACKEND"
  DATABASE_URL="sqlite:./data/shared_memory.db" ./target/release/server
) &
BACKEND_PID=$!

# ── Install & start Open WebUI in background ─────────────────────
if [ -z "$SKIP_OPENWEBUI" ]; then
  # Install open-webui automatically on first run
  if ! "$PYTHON" -c "import open_webui" 2>/dev/null; then
    echo "  Installing Open WebUI (first run only, this may take a minute)..."
    "$PYTHON" -m pip install open-webui --break-system-packages -q
  fi

  export PORT=3001
  export HOST="0.0.0.0"
  export OPENAI_API_BASE_URL="http://localhost:8080/v1"
  export OPENAI_API_KEY="sk-sharedllm"
  export WEBUI_AUTH="False"
  export CORS_ALLOW_ORIGIN="*"
  export DATA_DIR="$ROOT/.openwebui-data"
  mkdir -p "$DATA_DIR"

  "$PYTHON" -m open_webui serve --host "$HOST" --port "$PORT" \
    > /tmp/openwebui.log 2>&1 &
  OPENWEBUI_PID=$!
fi

# ── Brief pause for services to bind ─────────────────────────────
sleep 2

echo "  Backend      →  http://localhost:8080"
  echo "  Chat (WebUI) →  http://localhost:3001"
echo "  API          →  http://localhost:8080/api"
echo "  WebSocket    →  ws://localhost:8080/ws"
echo ""
if [ -n "$OPENWEBUI_PID" ]; then
  echo "  Open WebUI log: /tmp/openwebui.log"
fi
echo ""
echo "  Press Ctrl+C to stop everything."
echo ""

# ── Keep running until backend exits (or Ctrl+C) ─────────────────
wait $BACKEND_PID || true
