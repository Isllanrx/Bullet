use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use serde::Deserialize;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::{debug, info};

const DOH_TIMEOUT: Duration = Duration::from_secs(4);
const TCP_TIMEOUT: Duration = Duration::from_secs(4);

const DOH_PROVIDERS: [&str; 3] = [
    "https://dns.google/resolve",
    "https://cloudflare-dns.com/dns-query",
    "https://dns.quad9.net:5053/dns-query",
];

#[derive(Deserialize)]
struct DohResponse {
    #[serde(rename = "Answer", default)]
    answer: Vec<DohAnswer>,
}

#[derive(Deserialize)]
struct DohAnswer {
    #[serde(rename = "type", default)]
    kind: u16,
    #[serde(default)]
    data: String,
}

pub(super) async fn connect_with_doh_fallback(
    url: &str,
    config: WebSocketConfig,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
    let first_err =
        match tokio_tungstenite::connect_async_with_config(url, Some(config), false).await {
            Ok((stream, _)) => return Ok(stream),
            Err(WsError::Http(resp)) if resp.status().as_u16() == 409 => {
                return Err(WsError::Http(resp));
            }
            Err(e) => e,
        };

    let Some((host, port)) = parse_wss_host_port(url) else {
        return Err(first_err);
    };
    if host.eq_ignore_ascii_case("localhost") || host.parse::<IpAddr>().is_ok() {
        return Err(first_err);
    }

    debug!(host = %host, error = %first_err, "Party: direct relay connect failed; trying DoH fallback");
    let ips = resolve_doh(&host).await;
    for ip in ips {
        let addr = SocketAddr::new(ip, port);
        let Ok(Ok(tcp)) = tokio::time::timeout(TCP_TIMEOUT, TcpStream::connect(addr)).await else {
            continue;
        };
        let _ = tcp.set_nodelay(true); // ignore-ok: best-effort latency hint
        match tokio_tungstenite::client_async_tls_with_config(url, tcp, Some(config), None).await {
            Ok((stream, _)) => {
                info!(host = %host, ip = %ip, "Party: connected to relay via DoH fallback");
                return Ok(stream);
            }
            Err(WsError::Http(resp)) if resp.status().as_u16() == 409 => {
                return Err(WsError::Http(resp));
            }
            Err(e) => {
                debug!(host = %host, ip = %ip, error = %e, "Party: DoH candidate address failed");
            }
        }
    }

    Err(first_err)
}

fn parse_wss_host_port(url: &str) -> Option<(String, u16)> {
    let rest = url.strip_prefix("wss://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() {
        return None;
    }
    if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, tail) = bracketed.split_once(']')?;
        let port = tail
            .strip_prefix(':')
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(443);
        return (!host.is_empty()).then(|| (host.to_owned(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, port_str)) if !host.is_empty() => {
            let port = port_str.parse::<u16>().ok()?;
            Some((host.to_owned(), port))
        }
        _ => Some((authority.to_owned(), 443)),
    }
}

fn parse_doh_answers(body: &str) -> Vec<IpAddr> {
    let Ok(parsed) = serde_json::from_str::<DohResponse>(body) else {
        return Vec::new();
    };
    parsed
        .answer
        .into_iter()
        .filter(|a| matches!(a.kind, 1 | 28))
        .filter_map(|a| a.data.trim().parse::<IpAddr>().ok())
        .collect()
}

async fn resolve_doh(host: &str) -> Vec<IpAddr> {
    let Ok(client) = reqwest::Client::builder().timeout(DOH_TIMEOUT).build() else {
        return Vec::new();
    };
    let mut ips = Vec::new();
    for provider in DOH_PROVIDERS {
        for qtype in ["AAAA", "A"] {
            let url = format!("{provider}?name={host}&type={qtype}");
            let Ok(resp) = client
                .get(&url)
                .header("accept", "application/dns-json")
                .send()
                .await
            else {
                continue;
            };
            if !resp.status().is_success() {
                continue;
            }
            if let Ok(body) = resp.text().await {
                for ip in parse_doh_answers(&body) {
                    if !ips.contains(&ip) {
                        ips.push(ip);
                    }
                }
            }
        }
        if !ips.is_empty() {
            break;
        }
    }
    ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wss_host_and_port_only() {
        assert_eq!(
            parse_wss_host_port("wss://relay.example.workers.dev/room?key=abc"),
            Some(("relay.example.workers.dev".into(), 443))
        );
        assert_eq!(
            parse_wss_host_port("wss://relay.example.com:8443/room"),
            Some(("relay.example.com".into(), 8443))
        );
        assert_eq!(parse_wss_host_port("ws://127.0.0.1:8787/room"), None);
        assert_eq!(parse_wss_host_port("https://example.com"), None);
    }

    #[test]
    fn parses_doh_json_answers_filtering_only_a_and_aaaa() {
        let json = r#"{
            "Answer": [
                {"name": "x.dev", "type": 5, "data": "alias.dev."},
                {"name": "x.dev", "type": 1, "data": "104.21.1.2"},
                {"name": "x.dev", "type": 28, "data": "2606:4700:3030::6815:102"},
                {"name": "x.dev", "type": 1, "data": "not-an-ip"}
            ]
        }"#;
        let ips = parse_doh_answers(json);
        assert_eq!(ips.len(), 2);
        assert_eq!(ips[0].to_string(), "104.21.1.2");
        assert_eq!(ips[1].to_string(), "2606:4700:3030::6815:102");
    }
}
