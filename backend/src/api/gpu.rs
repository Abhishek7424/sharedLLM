use axum::{extract::State, response::IntoResponse, Json};
use std::sync::Arc;

use crate::{memory::aggregate_snapshot_async, AppState};

/// GET /api/gpu — current stats from all detected memory providers
pub async fn get_gpu_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut snapshots = aggregate_snapshot_async(&state.providers).await;

    // Fill in allocated_mb from DB — distribute allocations across providers
    // proportionally by total_mb, with GPU providers prioritised over system RAM.
    if let Ok(devices) = crate::db::queries::list_devices(&state.pool).await {
        let total_allocated: u64 = devices
            .iter()
            .filter(|d| d.status == "approved")
            .map(|d| d.allocated_memory_mb as u64)
            .sum();

        if total_allocated > 0 && !snapshots.is_empty() {
            let grand_total: u64 = snapshots.iter().map(|s| s.total_mb).sum();
            if grand_total > 0 {
                let mut remaining = total_allocated;
                let last_idx = snapshots.len() - 1;
                for (i, snap) in snapshots.iter_mut().enumerate() {
                    let share = if i == last_idx {
                        // Give all remaining to the last provider to avoid rounding loss
                        remaining
                    } else {
                        (total_allocated * snap.total_mb / grand_total).min(snap.total_mb)
                    };
                    snap.allocated_mb = share.min(snap.total_mb);
                    remaining = remaining.saturating_sub(share);
                }
            }
        }
    }

    Json(serde_json::json!({
        "providers": snapshots,
        "count": snapshots.len(),
    }))
}
