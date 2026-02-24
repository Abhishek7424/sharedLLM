use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{interval, sleep, Duration};
use which::which;

const OLLAMA_HOST: &str = "http://127.0.0.1:11434";
const HEALTH_INTERVAL_SECS: u64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub digest: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaListResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug)]
pub struct OllamaManager {
    pub host: String,
    client: Client,
    is_running: Arc<Mutex<bool>>,
    /// Handle to the child process we spawned (None if Ollama was already running externally)
    child: Arc<Mutex<Option<Child>>>,
}

impl OllamaManager {
    pub fn new(host: Option<String>) -> Self {
        OllamaManager {
            host: host.unwrap_or_else(|| OLLAMA_HOST.to_string()),
            client: Client::new(),
            is_running: Arc::new(Mutex::new(false)),
            child: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if Ollama HTTP server is reachable
    pub async fn is_healthy(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.host))
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Start Ollama as a background process if not already running
    pub async fn ensure_running(&self) -> Result<()> {
        if self.is_healthy().await {
            tracing::info!("Ollama is already running at {}", self.host);
            *self.is_running.lock().await = true;
            return Ok(());
        }

        // Find ollama binary
        let ollama_path = which("ollama").map_err(|_| {
            anyhow::anyhow!(
                "ollama binary not found in PATH. Install from https://ollama.ai"
            )
        })?;

        tracing::info!("Starting Ollama: {}", ollama_path.display());

        let child = Command::new(&ollama_path)
            .arg("serve")
            .spawn()?;

        // Store the child handle so it stays alive and can be managed
        *self.child.lock().await = Some(child);

        // Wait up to 10s for it to become healthy
        for attempt in 0..20 {
            sleep(Duration::from_millis(500)).await;
            if self.is_healthy().await {
                tracing::info!("Ollama started successfully");
                *self.is_running.lock().await = true;
                return Ok(());
            }
            tracing::debug!("Waiting for Ollama to start... attempt {}", attempt + 1);
        }

        // Failed to start — kill the child we spawned and clear the handle
        if let Some(mut c) = self.child.lock().await.take() {
            let _ = c.kill().await;
        }

        anyhow::bail!("Ollama failed to start within 10 seconds")
    }

    /// Kill the Ollama process we spawned (no-op if we didn't spawn it)
    pub async fn stop(&self) {
        if let Some(mut c) = self.child.lock().await.take() {
            tracing::info!("Stopping Ollama process");
            let _ = c.kill().await;
        }
        *self.is_running.lock().await = false;
    }

    /// Spawn a watchdog task that restarts Ollama if it crashes
    pub fn spawn_watchdog(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(HEALTH_INTERVAL_SECS));
            loop {
                ticker.tick().await;
                let healthy = self.is_healthy().await;

                // Check and release the lock before potentially calling ensure_running
                let was_running = {
                    let mut is_running = self.is_running.lock().await;
                    if !healthy && *is_running {
                        *is_running = false;
                        true
                    } else {
                        false
                    }
                };

                if was_running {
                    tracing::warn!("Ollama went down — attempting restart...");
                    if let Err(e) = self.ensure_running().await {
                        tracing::error!("Failed to restart Ollama: {}", e);
                    }
                }
            }
        });
    }

    /// List available local models
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>> {
        let resp = self
            .client
            .get(format!("{}/api/tags", self.host))
            .send()
            .await?
            .json::<OllamaListResponse>()
            .await?;
        Ok(resp.models)
    }

    /// Stream a model pull response as raw bytes
    pub async fn pull_model_stream(
        &self,
        model: &str,
    ) -> Result<reqwest::Response> {
        let resp = self
            .client
            .post(format!("{}/api/pull", self.host))
            .json(&serde_json::json!({ "name": model, "stream": true }))
            .send()
            .await?;
        Ok(resp)
    }

    /// Delete a model
    pub async fn delete_model(&self, model: &str) -> Result<()> {
        self.client
            .delete(format!("{}/api/delete", self.host))
            .json(&serde_json::json!({ "name": model }))
            .send()
            .await?;
        Ok(())
    }

    /// Proxy a raw request to Ollama (generate, chat, embeddings, etc.)
    pub async fn proxy_post(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(format!("{}{}", self.host, path))
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp)
    }
}
