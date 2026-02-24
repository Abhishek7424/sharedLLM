#!/usr/bin/env bash
set -e

ROOT="$(cd "$(dirname "$0")" && pwd)"
BACKEND="$ROOT/backend"
FRONTEND="$ROOT/frontend"

echo "=== SharedMem Network ==="
echo ""

# Check dependencies
command -v cargo >/dev/null 2>&1 || { echo "ERROR: Rust/cargo not found. Install from https://rustup.rs"; exit 1; }
command -v node >/dev/null 2>&1 || { echo "ERROR: Node.js not found. Install from https://nodejs.org"; exit 1; }

# Install frontend deps if needed
if [ ! -d "$FRONTEND/node_modules" ]; then
  echo "[1/3] Installing frontend dependencies..."
  (cd "$FRONTEND" && npm install)
fi

# Build frontend
echo "[2/3] Building frontend..."
(cd "$FRONTEND" && npm run build)

# Ensure DB directory exists
mkdir -p "$BACKEND/data"

# Start backend (serves frontend from ../frontend/dist)
echo "[3/3] Starting backend server..."
echo ""
echo "  Dashboard: http://localhost:8080"
echo "  API:       http://localhost:8080/api"
echo "  WebSocket: ws://localhost:8080/ws"
echo ""

(cd "$BACKEND" && DATABASE_URL="sqlite:./data/shared_memory.db" cargo run --release 2>&1)
