use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::db::{models::Device, queries};
use crate::ws::WsEvent;

/// Possible device states — all variants used in DB and future API endpoints
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Pending,
    Approved,
    Denied,
    Suspended,
    Offline,
}

impl DeviceStatus {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match self {
            DeviceStatus::Pending => "pending",
            DeviceStatus::Approved => "approved",
            DeviceStatus::Denied => "denied",
            DeviceStatus::Suspended => "suspended",
            DeviceStatus::Offline => "offline",
        }
    }
}

/// Permission service: handles approval, denial, role assignment
pub struct PermissionService {
    pool: SqlitePool,
    event_tx: broadcast::Sender<WsEvent>,
}

impl PermissionService {
    pub fn new(pool: SqlitePool, event_tx: broadcast::Sender<WsEvent>) -> Self {
        PermissionService { pool, event_tx }
    }

    /// Register a newly-discovered device (goes to pending unless trust_local_network is on)
    pub async fn register_device(
        &self,
        name: String,
        ip: String,
        mac: Option<String>,
        discovery_method: &str,
    ) -> anyhow::Result<Device> {
        // Check if device with this IP already exists
        if let Some(existing) = queries::get_device_by_ip(&self.pool, &ip).await? {
            // Update last_seen and return existing
            queries::update_device_last_seen(&self.pool, &existing.id).await?;
            return Ok(existing);
        }

        // Check trust_local_network setting
        let trust_all = queries::get_setting(&self.pool, "trust_local_network")
            .await?
            .map(|v| v == "true")
            .unwrap_or(false);

        let default_role = queries::get_setting(&self.pool, "default_role")
            .await?
            .unwrap_or_else(|| "role-guest".to_string());

        let mut device = Device::new(name.clone(), ip.clone(), mac, discovery_method);

        if trust_all {
            device.status = "approved".into();
            device.role_id = Some(default_role);
            tracing::info!("Auto-approved device {} (trust_local_network=true)", ip);
        } else {
            device.status = "pending".into();
            tracing::info!("Device {} is pending approval", ip);
        }

        queries::insert_device(&self.pool, &device).await?;

        // Broadcast event
        let event = if trust_all {
            WsEvent::DeviceApproved {
                device_id: device.id.clone(),
                name: device.name.clone(),
                ip: device.ip.clone(),
            }
        } else {
            WsEvent::DevicePendingApproval {
                device_id: device.id.clone(),
                name: device.name.clone(),
                ip: device.ip.clone(),
                discovery_method: discovery_method.to_string(),
            }
        };

        let _ = self.event_tx.send(event);
        Ok(device)
    }

    /// Approve a pending device and assign a role
    pub async fn approve_device(
        &self,
        device_id: &str,
        role_id: Option<&str>,
    ) -> anyhow::Result<Device> {
        let role = role_id.unwrap_or("role-guest");
        queries::update_device_status(&self.pool, device_id, "approved").await?;
        queries::update_device_role(&self.pool, device_id, role).await?;

        let device = queries::get_device(&self.pool, device_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))?;

        let _ = self.event_tx.send(WsEvent::DeviceApproved {
            device_id: device.id.clone(),
            name: device.name.clone(),
            ip: device.ip.clone(),
        });

        tracing::info!("Device {} approved with role {}", device.ip, role);
        Ok(device)
    }

    /// Deny a pending device
    pub async fn deny_device(&self, device_id: &str) -> anyhow::Result<()> {
        queries::update_device_status(&self.pool, device_id, "denied").await?;

        let _ = self.event_tx.send(WsEvent::DeviceDenied {
            device_id: device_id.to_string(),
        });

        tracing::info!("Device {} denied", device_id);
        Ok(())
    }

    /// Allocate memory to a device (enforces role limits)
    pub async fn allocate_memory(
        &self,
        device_id: &str,
        memory_mb: i64,
    ) -> anyhow::Result<()> {
        let device = queries::get_device(&self.pool, device_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Device not found"))?;

        if device.status != "approved" {
            anyhow::bail!("Device must be approved before allocating memory");
        }

        // Enforce role memory limit
        if let Some(role_id) = &device.role_id {
            if let Some(role) = queries::get_role(&self.pool, role_id).await? {
                if memory_mb > role.max_memory_mb {
                    anyhow::bail!(
                        "Requested {} MB exceeds role '{}' limit of {} MB",
                        memory_mb,
                        role.name,
                        role.max_memory_mb
                    );
                }
            }
        }

        queries::update_device_memory(&self.pool, device_id, memory_mb).await?;

        // Record allocation
        let alloc = crate::db::models::Allocation {
            id: Uuid::new_v4().to_string(),
            device_id: device_id.to_string(),
            memory_mb,
            provider: "system_ram".into(), // TODO: pick provider dynamically
            granted_at: chrono::Utc::now().to_rfc3339(),
            revoked_at: None,
        };
        queries::insert_allocation(&self.pool, &alloc).await?;

        let _ = self.event_tx.send(WsEvent::MemoryAllocated {
            device_id: device_id.to_string(),
            memory_mb,
        });

        tracing::info!("Allocated {} MB to device {}", memory_mb, device_id);
        Ok(())
    }
}
