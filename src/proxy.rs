use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::OnceLock;

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::AppState;

static PROXY_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn proxy_client() -> &'static reqwest::Client {
    PROXY_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build proxy client")
    })
}

const MAX_BODY_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Deserialize)]
pub struct ImgParams {
    pub u: Option<String>,
}

pub async fn proxy_image(
    State(state): State<AppState>,
    Query(params): Query<ImgParams>,
) -> Response {
    let Some(u) = params.u else {
        return (StatusCode::BAD_REQUEST, "missing u").into_response();
    };
    let Ok(parsed) = url::Url::parse(&u) else {
        return (StatusCode::BAD_REQUEST, "invalid url").into_response();
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return (StatusCode::BAD_REQUEST, "only http/https allowed").into_response();
    }
    let client = if state.allow_private_proxy {
        proxy_client()
    } else {
        let Some(addrs) = resolve_validated(&parsed).await else {
            return (StatusCode::BAD_GATEWAY, "blocked target").into_response();
        };
        let Some(host) = parsed.host_str() else {
            return (StatusCode::BAD_GATEWAY, "blocked target").into_response();
        };
        let Ok(client) = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(host, &addrs)
            .build()
        else {
            return (StatusCode::BAD_GATEWAY, "client build failed").into_response();
        };
        return fetch(&client, &parsed).await;
    };
    fetch(client, &parsed).await
}

async fn fetch(client: &reqwest::Client, parsed: &url::Url) -> Response {
    match client.get(parsed.as_str()).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return (StatusCode::BAD_GATEWAY, "upstream error").into_response();
            }
            if resp
                .content_length()
                .is_some_and(|len| len > MAX_BODY_BYTES)
            {
                return (StatusCode::PAYLOAD_TOO_LARGE, "image too large").into_response();
            }
            let content_type = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let bytes = match resp.bytes().await {
                Ok(b) if b.len() as u64 <= MAX_BODY_BYTES => b,
                Ok(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "image too large").into_response(),
                Err(_) => return (StatusCode::BAD_GATEWAY, "read failed").into_response(),
            };
            ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
        }
        Err(_) => (StatusCode::BAD_GATEWAY, "upstream unavailable").into_response(),
    }
}

async fn resolve_validated(parsed: &url::Url) -> Option<Vec<SocketAddr>> {
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs = tokio::net::lookup_host((host, port)).await.ok()?;
    let mut validated = Vec::new();
    for addr in addrs {
        if is_blocked_ip(addr.ip()) {
            return None;
        }
        validated.push(SocketAddr::new(addr.ip(), 0));
    }
    Some(validated)
}

fn is_v4_reserved(addr: Ipv4Addr) -> bool {
    addr.octets()[0] >= 240
}

fn is_v4_cgnat(addr: Ipv4Addr) -> bool {
    let octets = addr.octets();
    octets[0] == 100 && (octets[1] & 0xC0) == 0x40
}

fn is_v6_reserved(addr: Ipv6Addr) -> bool {
    let segments = addr.segments();
    segments[0] < 0x100 || (segments[0] & 0xFFC0) == 0xFEC0
}

fn is_v6_documentation(addr: Ipv6Addr) -> bool {
    let segments = addr.segments();
    segments[0] == 0x2001 && segments[1] == 0x0db8
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || is_v4_reserved(v4)
                || v4.is_documentation()
                || is_v4_cgnat(v4)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return v4.is_private()
                    || v4.is_loopback()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
                    || is_v4_reserved(v4)
                    || v4.is_documentation()
                    || is_v4_cgnat(v4);
            }
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_unspecified()
                || is_v6_reserved(v6)
                || is_v6_documentation(v6)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_loopback_and_link_local_v4() {
        for ip in [
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "127.0.0.1",
            "169.254.169.254",
        ] {
            assert!(is_blocked_ip(ip.parse().unwrap()), "{ip} should be blocked");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "93.184.216.34"] {
            assert!(
                !is_blocked_ip(ip.parse().unwrap()),
                "{ip} should be allowed"
            );
        }
    }

    #[test]
    fn blocks_loopback_ula_and_link_local_v6() {
        for ip in ["::1", "fc00::1", "fd00::1", "fe80::1"] {
            assert!(is_blocked_ip(ip.parse().unwrap()), "{ip} should be blocked");
        }
        for ip in ["2606:4700:4700::1111", "2001:4860:4860::8888"] {
            assert!(
                !is_blocked_ip(ip.parse().unwrap()),
                "{ip} should be allowed"
            );
        }
    }

    #[test]
    fn blocks_ipv4_mapped_private_ips() {
        for ip in ["::ffff:127.0.0.1", "::ffff:10.1.2.3", "::ffff:192.168.1.1"] {
            assert!(is_blocked_ip(ip.parse().unwrap()), "{ip} should be blocked");
        }
        assert!(!is_blocked_ip("::ffff:8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn blocks_cgnat_shared_space_with_boundaries() {
        for ip in ["100.64.0.1", "100.127.255.1", "::ffff:100.64.0.1"] {
            assert!(is_blocked_ip(ip.parse().unwrap()), "{ip} should be blocked");
        }
        for ip in ["100.63.255.1", "100.128.0.1"] {
            assert!(
                !is_blocked_ip(ip.parse().unwrap()),
                "{ip} should be allowed"
            );
        }
    }

    #[test]
    fn blocks_unspecified_reserved_broadcast_and_documentation() {
        for ip in [
            "0.0.0.0",
            "::",
            "255.255.255.255",
            "240.0.0.1",
            "192.0.2.1",
            "2001:db8::1",
            "::ffff:0.0.0.0",
            "::ffff:192.0.2.1",
        ] {
            assert!(is_blocked_ip(ip.parse().unwrap()), "{ip} should be blocked");
        }
    }
}
