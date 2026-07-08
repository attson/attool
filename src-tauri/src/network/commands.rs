use super::dns::resolve as run_dns;
use super::ping::{run_ping, PingResult};
use super::port::{check_port, parse_ports, PortCheckResult, PortStatus};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ---- ping ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    pub host: String,
    pub count: Option<u32>,
    pub timeout_seconds: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    pub raw_output: String,
    pub transmitted: Option<u32>,
    pub received: Option<u32>,
    pub loss_percent: Option<f32>,
    pub min_ms: Option<f32>,
    pub avg_ms: Option<f32>,
    pub max_ms: Option<f32>,
}

impl From<PingResult> for PingResponse {
    fn from(r: PingResult) -> Self {
        Self {
            raw_output: r.raw_output,
            transmitted: r.transmitted,
            received: r.received,
            loss_percent: r.loss_percent,
            min_ms: r.min_ms,
            avg_ms: r.avg_ms,
            max_ms: r.max_ms,
        }
    }
}

#[tauri::command]
pub async fn ping_host(request: PingRequest) -> Result<PingResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let count = request.count.unwrap_or(4).clamp(1, 20);
        let timeout = Duration::from_secs(request.timeout_seconds.unwrap_or(2).clamp(1, 30) as u64);
        run_ping(&request.host, count, timeout).map(PingResponse::from)
    })
    .await
    .map_err(|error| format!("ping 异常：{error}"))?
}

// ---- port ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortCheckRequest {
    pub host: String,
    pub ports: String,
    pub timeout_ms: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortCheckItem {
    pub port: u16,
    pub status: String, // "open" | "closed" | "timeout" | "error"
    pub message: Option<String>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortCheckResponse {
    pub host: String,
    pub results: Vec<PortCheckItem>,
}

fn status_str(status: &PortStatus) -> (&'static str, Option<String>) {
    match status {
        PortStatus::Open => ("open", None),
        PortStatus::Closed => ("closed", None),
        PortStatus::Timeout => ("timeout", None),
        PortStatus::Error(msg) => ("error", Some(msg.clone())),
    }
}

#[tauri::command]
pub async fn check_ports(request: PortCheckRequest) -> Result<PortCheckResponse, String> {
    let host = request.host.trim().to_string();
    if host.is_empty() {
        return Err("请输入主机名".to_string());
    }
    let ports = parse_ports(&request.ports)?;
    let timeout = Duration::from_millis(request.timeout_ms.unwrap_or(2000).clamp(200, 15_000) as u64);

    tauri::async_runtime::spawn_blocking(move || {
        let mut results = Vec::with_capacity(ports.len());
        for p in ports {
            let PortCheckResult {
                port,
                status,
                elapsed_ms,
            } = check_port(&host, p, timeout);
            let (s, msg) = status_str(&status);
            results.push(PortCheckItem {
                port,
                status: s.to_string(),
                message: msg,
                elapsed_ms,
            });
        }
        Ok::<_, String>(PortCheckResponse { host, results })
    })
    .await
    .map_err(|error| format!("端口扫描异常：{error}"))?
}

// ---- dns ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsResponse {
    pub v4: Vec<String>,
    pub v6: Vec<String>,
}

#[tauri::command]
pub async fn resolve_dns(host: String) -> Result<DnsResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let r = run_dns(&host)?;
        Ok(DnsResponse { v4: r.v4, v6: r.v6 })
    })
    .await
    .map_err(|error| format!("DNS 异常：{error}"))?
}
