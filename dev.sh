#!/usr/bin/env bash
# Dev mode: runs backend + frontend dev server in parallel
set -e

ROOT="$(cd "$(dirname "$0")" && pwd)"

echo "=== SharedMem Network (DEV) ==="
echo "  Frontend: http://localhost:5173"
echo "  Backend:  http://localhost:8080"
echo ""

# Ensure DB directory exists
mkdir -p "$ROOT/backend/data"

# Start backend in background
(cd "$ROOT/backend" && DATABASE_URL="sqlite:./data/shared_memory.db" cargo run 2>&1 | sed 's/^/[backend] /') &
BACKEND_PID=$!

# Start frontend dev server
(cd "$ROOT/frontend" && npm run dev 2>&1 | sed 's/^/[frontend] /') &
FRONTEND_PID=$!

# Trap Ctrl+C to kill both
trap "kill $BACKEND_PID $FRONTEND_PID 2>/dev/null; exit" INT TERM

wait
