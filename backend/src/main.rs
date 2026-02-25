mod api;
mod db;
mod discovery;
mod llama_cpp;
mod memory;
mod ollama;
mod permissions;
mod ws;

use anyhow::Result;
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use llama_cpp::LlamaCppManager;
use memory::MemoryProvider;
use ollama::OllamaManager;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::ws::WsEvent;

// ─── Open WebUI auto-start ────────────────────────────────────────────────────

async fn maybe_start_open_webui() {
    const PORT: u16 = 3001;
    const PYTHON: &str = "/opt/homebrew/bin/python3.12";

    // If something is already bound on the port, do nothing.
    if tokio::net::TcpStream::connect(("127.0.0.1", PORT)).await.is_ok() {
        tracing::info!("Open WebUI already running on port {}", PORT);
        return;
    }

    if !std::path::Path::new(PYTHON).exists() {
        tracing::warn!("Python 3.12 not found at {} — skipping Open WebUI auto-start", PYTHON);
        return;
    }

    // Resolve project root: binary is at <root>/backend/target/release/server
    let data_dir = std::env::var("OPENWEBUI_DATA_DIR").unwrap_or_else(|_| {
        std::env::current_exe()
            .ok()
            .and_then(|p| {
                // .../backend/target/release/server → go up 4 levels → project root
                p.ancestors().nth(4).map(|r| r.join(".openwebui-data").to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "/tmp/.openwebui-data".to_string())
    });

    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        tracing::warn!("Could not create Open WebUI data dir {}: {}", data_dir, e);
    }

    tracing::info!("Starting Open WebUI on port {} (data dir: {})", PORT, data_dir);

    // Use sh to redirect both stdout and stderr to the log file
    let script = format!(
        r#"exec {python} -m open_webui serve --host 0.0.0.0 --port {port} >> /tmp/openwebui.log 2>&1"#,
        python = PYTHON,
        port = PORT,
    );

    let mut cmd = tokio::process::Command::new("sh");
    cmd.args(["-c", &script])
        .env("OPENAI_API_BASE_URL", "http://localhost:8080/v1")
        .env("OPENAI_API_KEY", "sk-sharedllm")
        .env("WEBUI_AUTH", "False")
        .env("CORS_ALLOW_ORIGIN", "*")
        .env("DATA_DIR", &data_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    match cmd.spawn() {
        Ok(child) => {
            tracing::info!("Open WebUI spawned (pid {:?}), log: /tmp/openwebui.log", child.id());
            drop(child);
        }
        Err(e) => {
            tracing::warn!("Failed to spawn Open WebUI: {}", e);
        }
    }
}

// ─── App State ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub event_tx: broadcast::Sender<WsEvent>,
    pub providers: Vec<Arc<dyn MemoryProvider>>,
    pub ollama: Arc<OllamaManager>,
    pub llama_cpp: Arc<LlamaCppManager>,
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "shared_memory_backend=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("=== Shared Memory Network starting ===");

    // Database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data/shared_memory.db".to_string());
    let pool = db::init_pool(&db_url).await?;
    tracing::info!("Database ready");

    // Memory providers
    let providers = memory::detect_providers();
    tracing::info!("Detected {} memory provider(s)", providers.len());

    // WebSocket broadcast channel
    let (event_tx, _) = broadcast::channel::<WsEvent>(256);

    // Ollama manager
    let ollama_host = db::queries::get_setting(&pool, "ollama_host")
        .await
        .ok()
        .flatten();
    let ollama = Arc::new(OllamaManager::new(ollama_host));

    // llama.cpp manager (for distributed inference)
    let llama_cpp = Arc::new(LlamaCppManager::new(event_tx.clone()));
    tracing::info!(
        "llama-rpc-server: {}",
        if LlamaCppManager::find_rpc_server_bin().is_some() { "found" } else { "not found" }
    );
    tracing::info!(
        "llama-server: {}",
        if LlamaCppManager::find_inference_server_bin().is_some() { "found" } else { "not found" }
    );

    // Auto-start Ollama
    let auto_start = db::queries::get_setting(&pool, "auto_start_ollama")
        .await
        .unwrap_or(None)
        .map(|v| v == "true")
        .unwrap_or(true);

    if auto_start {
        match ollama.ensure_running().await {
            Ok(()) => {
                let _ = event_tx.send(WsEvent::OllamaStatus {
                    running: true,
                    host: ollama.host.clone(),
                });
            }
            Err(e) => {
                tracing::warn!("Ollama auto-start failed: {}. Continuing without it.", e);
                let _ = event_tx.send(WsEvent::OllamaStatus {
                    running: false,
                    host: ollama.host.clone(),
                });
            }
        }
        // Start watchdog
        ollama.clone().spawn_watchdog();
    }

    // Auto-start Open WebUI (non-blocking — it will take ~30s to warm up)
    tokio::spawn(maybe_start_open_webui());

    // mDNS: advertise this host
    let _mdns_daemon = discovery::advertise().ok();

    // mDNS: browse for other devices
    let mdns_enabled = db::queries::get_setting(&pool, "mdns_enabled")
        .await
        .unwrap_or(None)
        .map(|v| v == "true")
        .unwrap_or(true);

    if mdns_enabled {
        discovery::browse(event_tx.clone()).await.ok();
    }

    // App state
    let state = Arc::new(AppState {
        pool: pool.clone(),
        event_tx: event_tx.clone(),
        providers,
        ollama: ollama.clone(),
        llama_cpp: llama_cpp.clone(),
    });

    // Spawn GPU stats broadcaster (every 3 seconds)
    {
        let state_clone = state.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(3));
            loop {
                ticker.tick().await;
                let snapshots = memory::aggregate_snapshot_async(&state_clone.providers).await;
                let _ = state_clone.event_tx.send(WsEvent::MemoryStats { snapshots });
            }
        });
    }

    // mDNS device-auto-register task: listen for DeviceDiscovered events and register them
    {
        let pool_clone = pool.clone();
        let tx_clone = event_tx.clone();
        let mut rx = event_tx.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let WsEvent::DeviceDiscovered { ip, name, hostname: _, method } = event {
                    let svc = permissions::PermissionService::new(pool_clone.clone(), tx_clone.clone());
                    if let Err(e) = svc.register_device(name, ip, None, &method).await {
                        tracing::warn!("Failed to register discovered device: {}", e);
                    }
                }
            }
        });
    }

    // Build router
    let app = build_router(state);

    // Start server
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Server listening on http://{}", addr);
    tracing::info!("Dashboard: http://localhost:{}", port);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn openwebui_status_handler() -> axum::Json<serde_json::Value> {
    let running = tokio::net::TcpStream::connect("127.0.0.1:3001").await.is_ok();
    axum::Json(serde_json::json!({ "running": running }))
}

fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // WebSocket
        .route("/ws", get(api::ws_handler::ws_handler))
        // Devices
        .route("/api/devices", get(api::devices::list_devices))
        .route("/api/devices", post(api::devices::add_device))
        .route("/api/devices/:id", get(api::devices::get_device))
        .route("/api/devices/:id", delete(api::devices::delete_device))
        .route("/api/devices/:id/approve", post(api::devices::approve_device))
        .route("/api/devices/:id/deny", post(api::devices::deny_device))
        .route("/api/devices/:id/memory", patch(api::devices::allocate_memory))
        // GPU / Memory stats
        .route("/api/gpu", get(api::gpu::get_gpu_stats))
        // Models / Ollama
        .route("/api/models", get(api::models::list_models))
        .route("/api/models/pull", post(api::models::pull_model))
        .route("/api/models/:name", delete(api::models::delete_model))
        .route("/api/ollama/status", get(api::models::ollama_status))
        // Permissions / Roles
        .route("/api/permissions/roles", get(api::permissions::list_roles))
        .route("/api/permissions/roles", post(api::permissions::create_role))
        .route("/api/permissions/roles/:id", put(api::permissions::update_role))
        .route("/api/permissions/roles/:id", delete(api::permissions::delete_role))
        // Settings
        .route("/api/settings", get(api::settings::list_settings))
        .route("/api/settings/:key", put(api::settings::update_setting))
        // Inference backend config
        .route("/api/backends/config", get(api::backends::get_backend_config))
        .route("/api/backends/config", post(api::backends::set_backend_config))
        .route("/api/backends/models", get(api::backends::list_backend_models))
        // Cluster / Distributed inference
        .route("/api/cluster/status", get(api::cluster::cluster_status))
        .route("/api/cluster/model-check", get(api::cluster::model_check))
        .route("/api/cluster/inference/start", post(api::cluster::start_inference))
        .route("/api/cluster/inference/stop", post(api::cluster::stop_inference))
        .route("/api/cluster/inference/status", get(api::cluster::inference_status))
        .route("/api/cluster/rpc/start", post(api::cluster::start_rpc_server))
        .route("/api/cluster/rpc/stop", post(api::cluster::stop_rpc_server))
        // Binary installer (streams NDJSON progress)
        .route("/api/cluster/install-binaries", post(api::install::install_binaries))
        // OpenAI-compatible API proxy → llama-server (used by Open WebUI)
        .route("/v1/models", get(api::cluster::models_proxy))
        .route("/v1/chat/completions", post(api::cluster::chat_completions_proxy))
        // Open WebUI status (TCP probe)
        .route("/api/openwebui/status", get(openwebui_status_handler))
        // Agent install scripts
        .route("/agent/install", get(api::agent::install_script))
        .route("/agent/info", get(api::agent::agent_info))
        // Serve static frontend (production)
        .nest_service(
            "/",
            tower_http::services::ServeDir::new("../frontend/dist")
                .not_found_service(tower_http::services::ServeFile::new("../frontend/dist/index.html")),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
