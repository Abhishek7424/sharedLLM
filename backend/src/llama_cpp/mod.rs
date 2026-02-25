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

/// How well a model fits into the available cluster memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FitStatus {
    FitsLocally,
    FitsDistributed,
    PartialGpu,
    TooLarge,
}

/// Analysis of how a model will run across local + cluster memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAnalysis {
    pub model_size_mb: u64,
    pub estimated_layers: u32,
    pub local_free_mb: u64,
    pub cluster_free_mb: u64,
    pub total_available_mb: u64,
    pub fit_status: FitStatus,
    /// Recommended --n-gpu-layers value for llama-server.
    /// -1 means "all layers on GPU", 0 means "CPU only".
    pub recommended_n_gpu_layers: i32,
    pub recommended_ctx_size: u32,
    pub warnings: Vec<String>,
}

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

    /// Estimate llama.cpp layer count from model file size (MB).
    /// These are approximate heuristics based on common GGUF model families.
    fn estimate_layers(model_size_mb: u64) -> u32 {
        match model_size_mb {
            0..=2047   => 22, // ~1-3B
            2048..=5119  => 32, // ~7B
            5120..=9215  => 40, // ~13B
            9216..=20479 => 48, // ~30-34B
            20480..=40959 => 64, // ~40-65B
            _            => 80, // ~70B+
        }
    }

    /// Analyse how well a model fits into local + cluster memory.
    ///
    /// - `model_path`       – absolute path to the .gguf file (used for size).
    /// - `local_free_mb`    – free memory on this machine (GPU/unified).
    /// - `device_free_mbs`  – free memory per approved cluster device.
    pub fn analyze_model(
        model_path: &str,
        local_free_mb: u64,
        device_free_mbs: Vec<u64>,
    ) -> anyhow::Result<ModelAnalysis> {
        let model_size_mb = std::fs::metadata(model_path)
            .map(|m| m.len() / (1024 * 1024))
            .unwrap_or(0);

        if model_size_mb == 0 {
            return Err(anyhow!("Model file not found or empty: {}", model_path));
        }

        let estimated_layers = Self::estimate_layers(model_size_mb);
        let cluster_free_mb: u64 = device_free_mbs.iter().sum();
        let total_available_mb = local_free_mb + cluster_free_mb;

        let mut warnings: Vec<String> = Vec::new();

        // Leave 10% headroom when computing "usable" memory.
        let usable_local  = (local_free_mb  as f64 * 0.90) as u64;
        let usable_total  = (total_available_mb as f64 * 0.90) as u64;

        let fit_status = if model_size_mb <= usable_local {
            FitStatus::FitsLocally
        } else if model_size_mb <= usable_total && cluster_free_mb > 0 {
            FitStatus::FitsDistributed
        } else if model_size_mb <= total_available_mb {
            if cluster_free_mb == 0 {
                warnings.push(
                    "Add cluster devices to offload layers and fit this model".to_string(),
                );
            } else {
                warnings.push("Model may not fit — very tight on memory".to_string());
            }
            FitStatus::PartialGpu
        } else {
            warnings.push(format!(
                "Model needs ~{} GB but only {} GB available across cluster",
                (model_size_mb + 511) / 1024,
                (total_available_mb + 511) / 1024,
            ));
            FitStatus::TooLarge
        };

        // Recommended n_gpu_layers (-1 = all layers on GPU)
        let recommended_n_gpu_layers: i32 = match &fit_status {
            FitStatus::FitsLocally => -1,
            FitStatus::FitsDistributed => {
                // Local handles a proportional fraction of layers
                if total_available_mb > 0 {
                    let frac = local_free_mb as f64 / total_available_mb as f64;
                    (frac * estimated_layers as f64).round() as i32
                } else {
                    0
                }
            }
            FitStatus::PartialGpu => {
                // Put as many layers as local memory can hold
                if model_size_mb > 0 {
                    let frac = (local_free_mb as f64 / model_size_mb as f64).min(1.0);
                    (frac * estimated_layers as f64).round() as i32
                } else {
                    0
                }
            }
            FitStatus::TooLarge => 0,
        };

        // Recommended ctx_size based on remaining memory after model
        let remaining_mb = total_available_mb.saturating_sub(model_size_mb);
        let recommended_ctx_size: u32 = match remaining_mb {
            0..=1023   => 2048,
            1024..=2047 => 4096,
            2048..=4095 => 8192,
            _           => 16384,
        };

        Ok(ModelAnalysis {
            model_size_mb,
            estimated_layers,
            local_free_mb,
            cluster_free_mb,
            total_available_mb,
            fit_status,
            recommended_n_gpu_layers,
            recommended_ctx_size,
            warnings,
        })
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
        Self::find_binary("llama-server")
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
        let mut state = self.state.lock().await;

        // Reap any processes that have already exited so the UI shows correct status
        if let Some(child) = state.rpc_process.as_mut() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                state.rpc_process = None;
                tracing::info!("llama-rpc-server exited unexpectedly");
            }
        }
        if let Some(child) = state.inference_process.as_mut() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                state.inference_process = None;
                tracing::info!("llama-server exited unexpectedly");
            }
        }

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
        let mut state = self.state.lock().await;
        if let Some(child) = state.rpc_process.as_mut() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                state.rpc_process = None;
                return false;
            }
            true
        } else {
            false
        }
    }

    // ─── Inference server ─────────────────────────────────────────────────

    /// Start llama-server with the given model and optional RPC remote devices.
    ///
    /// `rpc_addresses` is a list of "ip:port" strings for remote devices
    /// (e.g. ["192.168.1.10:8181"]). Pass an empty list to run locally only.
    ///
    /// `n_gpu_layers`: -1 = all layers on GPU, 0 = CPU only, N = N layers on GPU.
    /// `ctx_size`: context window in tokens.
    pub async fn start_inference(
        &self,
        model_path: &str,
        rpc_addresses: Vec<String>,
        n_gpu_layers: i32,
        ctx_size: u32,
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
            "--ctx-size".to_string(),
            ctx_size.to_string(),
        ];

        // Map our -1 sentinel ("all layers") to a large number llama-server understands.
        // 0 means CPU-only (omit the flag to let llama-server default).
        match n_gpu_layers {
            -1 => {
                args.push("--n-gpu-layers".to_string());
                args.push("999".to_string()); // "all" for any model
            }
            n if n > 0 => {
                args.push("--n-gpu-layers".to_string());
                args.push(n.to_string());
            }
            _ => {
                // 0 = CPU only, no flag needed (llama-server defaults to 0)
            }
        }

        if !rpc_addresses.is_empty() {
            args.push("--rpc".to_string());
            args.push(rpc_addresses.join(","));
        }

        tracing::info!(
            "Starting llama-server: model={} rpc=[{}] port={} n_gpu_layers={} ctx={}",
            model_path,
            rpc_addresses.join(","),
            self.inference_port,
            n_gpu_layers,
            ctx_size,
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
        let mut state = self.state.lock().await;
        if let Some(child) = state.inference_process.as_mut() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                state.inference_process = None;
                return false;
            }
            true
        } else {
            false
        }
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

    /// Check if a remote device's RPC server is reachable.
    /// Uses a 2-second TCP connect timeout so offline devices don't block the UI.
    pub async fn probe_rpc_device(&self, ip: &str, port: u16) -> bool {
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio::net::TcpStream::connect(format!("{}:{}", ip, port)),
        )
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
    }
}
