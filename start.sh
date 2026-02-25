#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────
#  SharedLLM — start everything with one command
#    • Rust/Axum backend    → http://localhost:8080
#    • Frontend (built)     → served by backend at :8080
#    • Open WebUI (Chat)    → auto-started by backend on :3001
#
#  Usage:  ./start.sh
#  Stop:   Ctrl+C  (kills backend; Open WebUI is managed by backend)
# ─────────────────────────────────────────────────────────────────

set -e

ROOT="$(cd "$(dirname "$0")" && pwd)"
BACKEND="$ROOT/backend"
FRONTEND="$ROOT/frontend"
PYTHON="/opt/homebrew/bin/python3.12"

BACKEND_PID=""

cleanup() {
  echo ""
  echo "Shutting down..."
  [ -n "$BACKEND_PID" ] && kill "$BACKEND_PID" 2>/dev/null || true
  wait 2>/dev/null || true
  echo "Done."
}
trap cleanup INT TERM EXIT

# ── Dependency checks ─────────────────────────────────────────────
echo "=== SharedLLM ==="
echo ""

command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found. Install from https://rustup.rs"; exit 1; }
command -v node  >/dev/null 2>&1 || { echo "ERROR: node not found. Install from https://nodejs.org";  exit 1; }

if [ ! -f "$PYTHON" ]; then
  echo "WARN: Python 3.12 not found at $PYTHON"
  echo "      Chat/Open WebUI will not start automatically."
  echo "      Fix with: brew install python@3.12"
  echo ""
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

# ── Start backend (it auto-starts Open WebUI internally) ──────────
echo "[3/3] Starting backend..."
echo ""

(
  cd "$BACKEND"
  DATABASE_URL="sqlite:./data/shared_memory.db" \
  OPENWEBUI_DATA_DIR="$ROOT/.openwebui-data" \
    ./target/release/server
) &
BACKEND_PID=$!

sleep 2

echo "  Backend      →  http://localhost:8080"
echo "  Chat (WebUI) →  http://localhost:3001  (starting in background)"
echo "  API          →  http://localhost:8080/api"
echo "  WebSocket    →  ws://localhost:8080/ws"
echo "  Open WebUI log: /tmp/openwebui.log"
echo ""
echo "  Press Ctrl+C to stop."
echo ""

wait $BACKEND_PID || true
