use anyhow::Result;
use sqlx::SqlitePool;

use super::models::{Allocation, Device, Role, Setting};

// ─── Device queries ──────────────────────────────────────────────────────────

pub async fn list_devices(pool: &SqlitePool) -> Result<Vec<Device>> {
    let devices = sqlx::query_as::<_, Device>("SELECT * FROM devices ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(devices)
}

pub async fn get_device(pool: &SqlitePool, id: &str) -> Result<Option<Device>> {
    let device = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(device)
}

pub async fn get_device_by_ip(pool: &SqlitePool, ip: &str) -> Result<Option<Device>> {
    let device = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE ip = ?")
        .bind(ip)
        .fetch_optional(pool)
        .await?;
    Ok(device)
}

pub async fn insert_device(pool: &SqlitePool, d: &Device) -> Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO devices (id, name, ip, mac, hostname, platform, role_id, status, discovery_method, allocated_memory_mb, last_seen, first_seen, created_at, rpc_port, rpc_status, memory_total_mb, memory_free_mb)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&d.id)
    .bind(&d.name)
    .bind(&d.ip)
    .bind(&d.mac)
    .bind(&d.hostname)
    .bind(&d.platform)
    .bind(&d.role_id)
    .bind(&d.status)
    .bind(&d.discovery_method)
    .bind(d.allocated_memory_mb)
    .bind(&d.last_seen)
    .bind(&d.first_seen)
    .bind(&d.created_at)
    .bind(d.rpc_port)
    .bind(&d.rpc_status)
    .bind(d.memory_total_mb)
    .bind(d.memory_free_mb)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_device_status(pool: &SqlitePool, id: &str, status: &str) -> Result<()> {
    sqlx::query("UPDATE devices SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_device_role(pool: &SqlitePool, id: &str, role_id: &str) -> Result<()> {
    sqlx::query("UPDATE devices SET role_id = ? WHERE id = ?")
        .bind(role_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_device_memory(pool: &SqlitePool, id: &str, memory_mb: i64) -> Result<()> {
    sqlx::query("UPDATE devices SET allocated_memory_mb = ? WHERE id = ?")
        .bind(memory_mb)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_device_last_seen(pool: &SqlitePool, id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE devices SET last_seen = ? WHERE id = ?")
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_device_rpc_status(pool: &SqlitePool, id: &str, rpc_status: &str) -> Result<()> {
    sqlx::query("UPDATE devices SET rpc_status = ? WHERE id = ?")
        .bind(rpc_status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_device(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM devices WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ─── Role queries ─────────────────────────────────────────────────────────────

pub async fn list_roles(pool: &SqlitePool) -> Result<Vec<Role>> {
    let roles = sqlx::query_as::<_, Role>("SELECT * FROM roles ORDER BY trust_level DESC")
        .fetch_all(pool)
        .await?;
    Ok(roles)
}

pub async fn get_role(pool: &SqlitePool, id: &str) -> Result<Option<Role>> {
    let role = sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(role)
}

pub async fn upsert_role(pool: &SqlitePool, r: &Role) -> Result<()> {
    sqlx::query(
        "INSERT INTO roles (id, name, max_memory_mb, can_pull_models, trust_level, created_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           max_memory_mb = excluded.max_memory_mb,
           can_pull_models = excluded.can_pull_models,
           trust_level = excluded.trust_level",
    )
    .bind(&r.id)
    .bind(&r.name)
    .bind(r.max_memory_mb)
    .bind(r.can_pull_models)
    .bind(r.trust_level)
    .bind(&r.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_role(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM roles WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ─── Allocation queries ───────────────────────────────────────────────────────

pub async fn insert_allocation(pool: &SqlitePool, a: &Allocation) -> Result<()> {
    sqlx::query(
        "INSERT INTO allocations (id, device_id, memory_mb, provider, granted_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&a.id)
    .bind(&a.device_id)
    .bind(a.memory_mb)
    .bind(&a.provider)
    .bind(&a.granted_at)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn list_allocations_for_device(
    pool: &SqlitePool,
    device_id: &str,
) -> Result<Vec<Allocation>> {
    let allocs = sqlx::query_as::<_, Allocation>(
        "SELECT * FROM allocations WHERE device_id = ? ORDER BY granted_at DESC",
    )
    .bind(device_id)
    .fetch_all(pool)
    .await?;
    Ok(allocs)
}

// ─── Settings queries ─────────────────────────────────────────────────────────

pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row = sqlx::query_as::<_, Setting>("SELECT * FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|s| s.value))
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_settings(pool: &SqlitePool) -> Result<Vec<Setting>> {
    let settings = sqlx::query_as::<_, Setting>("SELECT * FROM settings")
        .fetch_all(pool)
        .await?;
    Ok(settings)
}

