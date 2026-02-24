use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::broadcast;

use crate::ws::WsEvent;

const SERVICE_TYPE: &str = "_sharedmem._tcp.local.";
const SERVICE_NAME: &str = "SharedMemoryHost";
const API_PORT: u16 = 8080;

/// Discovered device info from mDNS
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiscoveredDevice {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub hostname: String,
}

/// Start mDNS advertisement so other devices can find this host
pub fn advertise() -> Result<ServiceDaemon> {
    let mdns = ServiceDaemon::new()?;

    // Get local hostname
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "shared-memory-host".to_string());

    // Get local IP
    let ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());

    let instance_name = format!("{SERVICE_NAME}.{SERVICE_TYPE}");

    let service_info = ServiceInfo::new(
        SERVICE_TYPE,
        SERVICE_NAME,
        &format!("{hostname}.local."),
        ip.as_str(),
        API_PORT,
        None,
    )?;

    mdns.register(service_info)?;
    tracing::info!("mDNS: advertising {} at {}:{}", instance_name, ip, API_PORT);

    Ok(mdns)
}

/// Browse for other SharedMemory devices on the LAN.
/// Sends discovered devices via the WsEvent broadcast channel.
/// Self-exclusion: devices advertising from our own IP or with the canonical
/// `SharedMemoryHost` instance name are ignored.
pub async fn browse(event_tx: broadcast::Sender<WsEvent>) -> Result<()> {
    let mdns = ServiceDaemon::new()?;
    let receiver = mdns.browse(SERVICE_TYPE)?;

    // Determine our own local IP once so we can skip it in the browse loop
    let own_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_default();

    tracing::info!("mDNS: browsing for {} devices (own IP: {})", SERVICE_TYPE, own_ip);

    tokio::task::spawn_blocking(move || {
        loop {
            match receiver.recv() {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let addresses = info.get_addresses();
                    if let Some(addr) = addresses.iter().next() {
                        let ip = addr.to_string();

                        // Skip ourselves — same IP means it's our own advertisement
                        if ip == own_ip {
                            tracing::debug!("mDNS: ignoring self-advertisement from {}", ip);
                            continue;
                        }

                        let device = DiscoveredDevice {
                            name: info.get_fullname().to_string(),
                            ip: ip.clone(),
                            port: info.get_port(),
                            hostname: info.get_hostname().to_string(),
                        };
                        tracing::info!("mDNS: discovered device at {}", device.ip);
                        let _ = event_tx.send(WsEvent::DeviceDiscovered {
                            ip: device.ip.clone(),
                            name: device.name.clone(),
                            hostname: device.hostname.clone(),
                            method: "mdns".into(),
                        });
                    }
                }
                Ok(ServiceEvent::ServiceRemoved(_, fullname)) => {
                    tracing::info!("mDNS: device removed: {}", fullname);
                    let _ = event_tx.send(WsEvent::DeviceOffline {
                        name: fullname.clone(),
                    });
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("mDNS browse error: {}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}
