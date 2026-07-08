use std::process::Command;
use std::time::Duration;

pub struct PingResult {
    pub raw_output: String,
    pub transmitted: Option<u32>,
    pub received: Option<u32>,
    pub loss_percent: Option<f32>,
    pub avg_ms: Option<f32>,
    pub min_ms: Option<f32>,
    pub max_ms: Option<f32>,
}

/// Shell out to the system `ping` command. Cross-platform behavior:
/// - Linux/macOS:   `ping -c <count> -W <sec> host`
/// - Windows:       `ping -n <count> -w <ms> host`
pub fn run_ping(host: &str, count: u32, timeout: Duration) -> Result<PingResult, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("请输入目标主机".to_string());
    }
    if count == 0 || count > 20 {
        return Err("count 必须在 1..=20".to_string());
    }

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("ping");
        c.arg("-n")
            .arg(count.to_string())
            .arg("-w")
            .arg(timeout.as_millis().to_string())
            .arg(host);
        c
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = Command::new("ping");
        c.arg("-c")
            .arg(count.to_string())
            .arg("-W")
            .arg(timeout.as_secs().max(1).to_string())
            .arg(host);
        c
    };

    let output = cmd
        .output()
        .map_err(|error| format!("启动 ping 失败：{error}"))?;

    let raw = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);

    let stats = parse_stats(&raw);
    Ok(PingResult {
        raw_output: raw,
        transmitted: stats.0,
        received: stats.1,
        loss_percent: stats.2,
        min_ms: stats.3,
        avg_ms: stats.4,
        max_ms: stats.5,
    })
}

/// Try to extract (transmitted, received, loss%, min, avg, max) from either style of ping summary.
fn parse_stats(
    output: &str,
) -> (
    Option<u32>,
    Option<u32>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
) {
    let mut tx = None;
    let mut rx = None;
    let mut loss = None;
    let mut min = None;
    let mut avg = None;
    let mut max = None;

    // e.g. "4 packets transmitted, 4 received, 0% packet loss"  (linux/macos)
    // or   "Packets: Sent = 4, Received = 4, Lost = 0 (0% loss)" (windows)
    if let Some(cap) = regex::Regex::new(r"(\d+)\s+packets transmitted[^\d]+(\d+)\s+(?:packets\s+)?received[^\d]+(\d+(?:\.\d+)?)\s*%")
        .ok()
        .and_then(|r| r.captures(output))
    {
        tx = cap.get(1).and_then(|m| m.as_str().parse().ok());
        rx = cap.get(2).and_then(|m| m.as_str().parse().ok());
        loss = cap.get(3).and_then(|m| m.as_str().parse().ok());
    } else if let Some(cap) = regex::Regex::new(r"Sent\s*=\s*(\d+).*Received\s*=\s*(\d+).*\((\d+)%\s+loss\)")
        .ok()
        .and_then(|r| r.captures(output))
    {
        tx = cap.get(1).and_then(|m| m.as_str().parse().ok());
        rx = cap.get(2).and_then(|m| m.as_str().parse().ok());
        loss = cap.get(3).and_then(|m| m.as_str().parse::<u32>().ok()).map(|v| v as f32);
    }

    // e.g. "round-trip min/avg/max/stddev = 12.345/23.456/34.567/1.234 ms"
    // or   "rtt min/avg/max/mdev = ..."
    if let Some(cap) = regex::Regex::new(
        r"(?:round-trip|rtt)\s+min/avg/max(?:/(?:stddev|mdev))?\s*=\s*(\d+(?:\.\d+)?)/(\d+(?:\.\d+)?)/(\d+(?:\.\d+)?)",
    )
    .ok()
    .and_then(|r| r.captures(output))
    {
        min = cap.get(1).and_then(|m| m.as_str().parse().ok());
        avg = cap.get(2).and_then(|m| m.as_str().parse().ok());
        max = cap.get(3).and_then(|m| m.as_str().parse().ok());
    } else if let Some(cap) = regex::Regex::new(
        r"Minimum\s*=\s*(\d+)ms.*Maximum\s*=\s*(\d+)ms.*Average\s*=\s*(\d+)ms",
    )
    .ok()
    .and_then(|r| r.captures(output))
    {
        min = cap.get(1).and_then(|m| m.as_str().parse().ok());
        max = cap.get(2).and_then(|m| m.as_str().parse().ok());
        avg = cap.get(3).and_then(|m| m.as_str().parse().ok());
    }

    (tx, rx, loss, min, avg, max)
}

#[cfg(test)]
mod tests {
    use super::parse_stats;

    #[test]
    fn parses_macos_summary() {
        let out = r#"
PING example.com (93.184.216.34): 56 data bytes
64 bytes from 93.184.216.34: icmp_seq=0 ttl=57 time=12.345 ms
--- example.com ping statistics ---
4 packets transmitted, 4 packets received, 0.0% packet loss
round-trip min/avg/max/stddev = 12.345/23.456/34.567/1.234 ms
"#;
        let (tx, rx, loss, min, avg, max) = parse_stats(out);
        assert_eq!(tx, Some(4));
        assert_eq!(rx, Some(4));
        assert_eq!(loss, Some(0.0));
        assert_eq!(min, Some(12.345));
        assert_eq!(avg, Some(23.456));
        assert_eq!(max, Some(34.567));
    }

    #[test]
    fn parses_linux_summary() {
        let out = r#"
--- example.com ping statistics ---
5 packets transmitted, 5 received, 0% packet loss, time 4005ms
rtt min/avg/max/mdev = 12.100/13.500/14.900/0.900 ms
"#;
        let (tx, rx, loss, min, avg, max) = parse_stats(out);
        assert_eq!(tx, Some(5));
        assert_eq!(rx, Some(5));
        assert_eq!(loss, Some(0.0));
        assert_eq!(min, Some(12.1));
        assert_eq!(avg, Some(13.5));
        assert_eq!(max, Some(14.9));
    }
}
