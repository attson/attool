use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum PortStatus {
    Open,
    Closed,
    Timeout,
    Error(String),
}

pub struct PortCheckResult {
    pub port: u16,
    pub status: PortStatus,
    pub elapsed_ms: u128,
}

/// Attempt a TCP connect to `host:port` with the given timeout. This is a simple
/// probe — a successful TCP handshake = "open". "Closed" = active reset (ECONNREFUSED).
/// A timeout doesn't distinguish filtered vs actually silent — just "we couldn't reach it".
pub fn check_port(host: &str, port: u16, timeout: Duration) -> PortCheckResult {
    let start = Instant::now();
    // Resolve host:port to socket addresses; use the first one that connects.
    let addrs: Vec<SocketAddr> = match format!("{host}:{port}").to_socket_addrs() {
        Ok(it) => it.collect(),
        Err(error) => {
            return PortCheckResult {
                port,
                status: PortStatus::Error(format!("解析地址失败：{error}")),
                elapsed_ms: start.elapsed().as_millis(),
            };
        }
    };

    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_) => {
                return PortCheckResult {
                    port,
                    status: PortStatus::Open,
                    elapsed_ms: start.elapsed().as_millis(),
                };
            }
            Err(err) => match err.kind() {
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                    return PortCheckResult {
                        port,
                        status: PortStatus::Timeout,
                        elapsed_ms: start.elapsed().as_millis(),
                    };
                }
                std::io::ErrorKind::ConnectionRefused => {
                    return PortCheckResult {
                        port,
                        status: PortStatus::Closed,
                        elapsed_ms: start.elapsed().as_millis(),
                    };
                }
                _ => {
                    // Try next resolved addr (e.g. v4 vs v6)
                    continue;
                }
            },
        }
    }

    PortCheckResult {
        port,
        status: PortStatus::Timeout,
        elapsed_ms: start.elapsed().as_millis(),
    }
}

/// Parse a "ports spec" like `80,443,8000-8010` into a deduped, ordered list of ports.
/// Rejects >1000 ports to avoid accidental full-range scans.
pub fn parse_ports(spec: &str) -> Result<Vec<u16>, String> {
    let mut out: Vec<u16> = Vec::new();
    for piece in spec.split(',') {
        let s = piece.trim();
        if s.is_empty() {
            continue;
        }
        if let Some((lo, hi)) = s.split_once('-') {
            let lo: u16 = lo.trim().parse().map_err(|_| format!("非法端口：{lo}"))?;
            let hi: u16 = hi.trim().parse().map_err(|_| format!("非法端口：{hi}"))?;
            if lo > hi {
                return Err(format!("端口范围颠倒：{lo}-{hi}"));
            }
            for p in lo..=hi {
                out.push(p);
            }
        } else {
            let p: u16 = s.parse().map_err(|_| format!("非法端口：{s}"))?;
            out.push(p);
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.len() > 1000 {
        return Err(format!(
            "一次最多扫 1000 个端口，当前 {}。缩小范围或分批",
            out.len()
        ));
    }
    if out.is_empty() {
        return Err("请输入至少一个端口".to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::parse_ports;

    #[test]
    fn parses_single_and_range() {
        let ports = parse_ports("80, 443, 8000-8003").unwrap();
        assert_eq!(ports, vec![80, 443, 8000, 8001, 8002, 8003]);
    }

    #[test]
    fn dedupes_and_sorts() {
        let ports = parse_ports("443, 80, 443, 80-81").unwrap();
        assert_eq!(ports, vec![80, 81, 443]);
    }

    #[test]
    fn rejects_reversed_range() {
        assert!(parse_ports("100-90").is_err());
    }

    #[test]
    fn rejects_non_numeric() {
        assert!(parse_ports("abc").is_err());
    }

    #[test]
    fn rejects_huge_range() {
        assert!(parse_ports("1-2000").is_err());
    }
}
