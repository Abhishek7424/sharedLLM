use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, Mutex};
use which::which;

use crate::ws::WsEvent;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceSessionInfo {
    pub id: String,
    pub model_path: String,
    pub status: String, // starting | running | stopped | error
    pub rpc_devices: Vec<String>, // "ip:port" strings
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaCppStatus {
    pub rpc_server_running: bool,
    pub inference_running: bool,
    pub rpc_server_bin: bool,
    pub inference_server_bin: bool,
    pub rpc_port: u16,
    pub inference_port: u16,
    pub current_session: Option<InferenceSessionInfo>,
}

// ─── Internal state ──────────────────────────────────────────────────────────

struct LlamaCppState {
    rpc_process: Option<Child>,
    inference_process: Option<Child>,
    current_session: Option<InferenceSessionInfo>,
}

// ─── Manager ─────────────────────────────────────────────────────────────────

pub struct LlamaCppManager {
    pub rpc_port: u16,
    pub inference_port: u16,
    pub client: Client,
    state: Arc<Mutex<LlamaCppState>>,
    event_tx: broadcast::Sender<WsEvent>,
}

impl LlamaCppManager {
    pub fn new(event_tx: broadcast::Sender<WsEvent>) -> Self {
        LlamaCppManager {
            rpc_port: 8181,
            inference_port: 8282,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            state: Arc::new(Mutex::new(LlamaCppState {
                rpc_process: None,
                inference_process: None,
                current_session: None,
            })),
            event_tx,
        }
    }

    // ─── Binary discovery ─────────────────────────────────────────────────

    /// Find a binary in PATH or ~/.sharedmem/bin/
    fn find_binary(name: &str) -> Option<PathBuf> {
        // First try PATH
        if let Ok(path) = which(name) {
            return Some(path);
        }
        // Then try ~/.sharedmem/bin/
        if let Ok(home) = std::env::var("HOME") {
            let path = PathBuf::from(home).join(".sharedmem").join("bin").join(name);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    pub fn find_rpc_server_bin() -> Option<PathBuf> {
        Self::find_binary("llama-rpc-server")
    }

    pub fn find_inference_server_bin() -> Option<PathBuf> {
        // Try both naming conventions
        Self::find_binary("llama-server").or_else(|| Self::find_binary("llama-cli"))
    }

    pub fn get_status_sync(
        rpc_running: bool,
        inf_running: bool,
        rpc_port: u16,
        inf_port: u16,
        session: Option<InferenceSessionInfo>,
    ) -> LlamaCppStatus {
        LlamaCppStatus {
            rpc_server_running: rpc_running,
            inference_running: inf_running,
            rpc_server_bin: Self::find_rpc_server_bin().is_some(),
            inference_server_bin: Self::find_inference_server_bin().is_some(),
            rpc_port,
            inference_port: inf_port,
            current_session: session,
        }
    }

    pub async fn get_status(&self) -> LlamaCppStatus {
        let state = self.state.lock().await;
        LlamaCppStatus {
            rpc_server_running: state.rpc_process.is_some(),
            inference_running: state.inference_process.is_some(),
            rpc_server_bin: Self::find_rpc_server_bin().is_some(),
            inference_server_bin: Self::find_inference_server_bin().is_some(),
            rpc_port: self.rpc_port,
            inference_port: self.inference_port,
            current_session: state.current_session.clone(),
        }
    }

    // ─── Local RPC server ─────────────────────────────────────────────────

    /// Start the local llama-rpc-server so this host's GPU can be used by other
    /// machines in the cluster.
    pub async fn start_rpc_server(&self) -> Result<()> {
        let binary = Self::find_rpc_server_bin()
            .ok_or_else(|| anyhow!(
                "llama-rpc-server not found. Install llama.cpp and add it to your PATH, \
                 or place it in ~/.sharedmem/bin/"
            ))?;

        let mut state = self.state.lock().await;

        if state.rpc_process.is_some() {
            tracing::debug!("llama-rpc-server already running");
            return Ok(());
        }

        tracing::info!("Starting llama-rpc-server on port {}", self.rpc_port);
        let child = Command::new(binary)
            .args(["--host", "0.0.0.0", "--port", &self.rpc_port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        state.rpc_process = Some(child);

        let _ = self.event_tx.send(WsEvent::RpcServerReady {
            port: self.rpc_port as i64,
        });

        Ok(())
    }

    pub async fn stop_rpc_server(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        if let Some(mut child) = state.rpc_process.take() {
            let _ = child.kill().await;
            tracing::info!("llama-rpc-server stopped");
            let _ = self.event_tx.send(WsEvent::RpcServerOffline);
        }
        Ok(())
    }

    pub async fn is_rpc_running(&self) -> bool {
        let state = self.state.lock().await;
        state.rpc_process.is_some()
    }

    // ─── Inference server ─────────────────────────────────────────────────

    /// Start llama-server with the given model and optional RPC remote devices.
    ///
    /// `rpc_addresses` is a list of "ip:port" strings for remote devices
    /// (e.g. ["192.168.1.10:8181"]). Pass an empty list to run locally only.
    pub async fn start_inference(
        &self,
        model_path: &str,
        rpc_addresses: Vec<String>,
    ) -> Result<()> {
        let binary = Self::find_inference_server_bin()
            .ok_or_else(|| anyhow!(
                "llama-server not found. Install llama.cpp and add it to your PATH, \
                 or place it in ~/.sharedmem/bin/"
            ))?;

        let mut state = self.state.lock().await;

        // Kill existing inference if running
        if let Some(mut child) = state.inference_process.take() {
            let _ = child.kill().await;
        }
        if let Some(session) = state.current_session.take() {
            let _ = self.event_tx.send(WsEvent::InferenceStopped {
                session_id: session.id,
            });
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();

        let mut args = vec![
            "-m".to_string(),
            model_path.to_string(),
            "--port".to_string(),
            self.inference_port.to_string(),
            "--host".to_string(),
            "0.0.0.0".to_string(),
            // Sensible defaults
            "--ctx-size".to_string(),
            "4096".to_string(),
        ];

        if !rpc_addresses.is_empty() {
            args.push("--rpc".to_string());
            args.push(rpc_addresses.join(","));
        }

        tracing::info!(
            "Starting llama-server: model={} rpc=[{}] port={}",
            model_path,
            rpc_addresses.join(","),
            self.inference_port
        );

        let child = Command::new(&binary)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let session = InferenceSessionInfo {
            id: session_id.clone(),
            model_path: model_path.to_string(),
            status: "starting".to_string(),
            rpc_devices: rpc_addresses.clone(),
            started_at,
        };

        state.inference_process = Some(child);
        state.current_session = Some(session);

        let _ = self.event_tx.send(WsEvent::InferenceStarted {
            session_id,
            model: model_path.to_string(),
            devices: rpc_addresses,
        });

        Ok(())
    }

    pub async fn stop_inference(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        if let Some(mut child) = state.inference_process.take() {
            let _ = child.kill().await;
            tracing::info!("llama-server stopped");
        }
        if let Some(session) = state.current_session.take() {
            let _ = self.event_tx.send(WsEvent::InferenceStopped {
                session_id: session.id,
            });
        }
        Ok(())
    }

    pub async fn is_inference_running(&self) -> bool {
        let state = self.state.lock().await;
        state.inference_process.is_some()
    }

    pub async fn get_current_session(&self) -> Option<InferenceSessionInfo> {
        let state = self.state.lock().await;
        state.current_session.clone()
    }

    /// Base URL for the inference server
    pub fn inference_base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.inference_port)
    }

    /// Health check — poll /health on the inference server
    pub async fn inference_is_healthy(&self) -> bool {
        self.client
            .get(format!("{}/health", self.inference_base_url()))
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Check if a remote device's RPC server is reachable
    pub async fn probe_rpc_device(&self, ip: &str, port: u16) -> bool {
        // Just try a TCP connect — we don't need HTTP here
        tokio::net::TcpStream::connect(format!("{}:{}", ip, port))
            .await
            .is_ok()
    }
}
