use std::net::{IpAddr, ToSocketAddrs};

pub struct DnsResult {
    pub v4: Vec<String>,
    pub v6: Vec<String>,
}

/// Resolve a hostname to IPv4 + IPv6 addresses using the system resolver.
/// Note: this uses `getaddrinfo` under the hood (via std::net), so it only
/// returns A/AAAA records — no MX/TXT/CNAME/etc. Adding those would require
/// a real DNS resolver crate (e.g. hickory-resolver), which is a new dep.
pub fn resolve(host: &str) -> Result<DnsResult, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("请输入主机名".to_string());
    }
    // Add a dummy port so ToSocketAddrs accepts a bare host.
    let iter = format!("{host}:0")
        .to_socket_addrs()
        .map_err(|error| format!("解析失败：{error}"))?;

    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    for addr in iter {
        match addr.ip() {
            IpAddr::V4(ip) => {
                let s = ip.to_string();
                if !v4.contains(&s) {
                    v4.push(s);
                }
            }
            IpAddr::V6(ip) => {
                let s = ip.to_string();
                if !v6.contains(&s) {
                    v6.push(s);
                }
            }
        }
    }
    Ok(DnsResult { v4, v6 })
}
