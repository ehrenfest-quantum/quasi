// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Runtime availability tracking for self-hosted providers.
//!
//! Cloud providers are considered live whenever their API key is present
//! (see `rotation::provider_has_key`) — we never probe them, to avoid
//! wasting quota. Self-hosted providers (those with `Provider::url_env_var`,
//! e.g. Ollama on a tailnet) are probed with a cheap TCP connect and the
//! result is cached for `CACHE_TTL` so `pick_model` is not probing on
//! every call.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use crate::config::get_provider;

/// How long a probe result (or a forced-down mark) stays valid.
const CACHE_TTL: Duration = Duration::from_secs(60);

/// Timeout for a single TCP connect attempt.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

static CACHE: OnceLock<Mutex<HashMap<String, (Instant, bool)>>> = OnceLock::new();

fn lock_cache() -> MutexGuard<'static, HashMap<String, (Instant, bool)>> {
    CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Is this provider currently usable? Cached.
///
/// Providers without a `url_env_var` (ordinary cloud providers) always
/// return `true` without any network traffic — key presence is already
/// handled by `rotation::provider_has_key`. Providers with a `url_env_var`
/// are probed at most once per `CACHE_TTL`.
pub fn is_provider_live(provider_id: &str) -> bool {
    let provider = match get_provider(provider_id) {
        Some(p) => p,
        // Unknown providers are rejected by `provider_has_key`; nothing to probe.
        None => return true,
    };
    let url_var = match provider.url_env_var {
        Some(v) => v,
        // Ordinary cloud provider: never probed.
        None => return true,
    };

    let url = std::env::var(url_var).unwrap_or_default();
    if url.trim().is_empty() {
        return false;
    }

    {
        let cache = lock_cache();
        if let Some((at, live)) = cache.get(provider_id) {
            if at.elapsed() < CACHE_TTL {
                return *live;
            }
        }
    }

    let live = probe(&url);
    lock_cache().insert(provider_id.to_string(), (Instant::now(), live));
    live
}

/// Record that a provider just failed at the connection level.
///
/// Forces the provider's cache entry to `false` for `CACHE_TTL`, so a
/// mid-flight transport failure immediately takes it out of selection
/// instead of waiting for the previous probe to expire. No-op for cloud
/// providers, which are always reported live.
pub fn mark_provider_down(provider_id: &str) {
    if let Some(p) = get_provider(provider_id) {
        if p.url_env_var.is_some() {
            lock_cache().insert(provider_id.to_string(), (Instant::now(), false));
        }
    }
}

/// Cheap liveness probe for a self-hosted endpoint: a plain TCP connect to
/// the host:port parsed out of the configured URL.
///
/// We deliberately use a blocking TCP connect instead of an async HTTP GET
/// on `/v1/models`: it needs no TLS handshake and works from the synchronous
/// `pick_model` path, where availability is consulted. The cost is bounded —
/// at most `PROBE_TIMEOUT` per resolved address, at most once per `CACHE_TTL`
/// per provider. Because `pick_model` is called from async contexts, the
/// connect is wrapped in `tokio::task::block_in_place` when a multi-thread
/// runtime is active, so the worker thread stays available for other tasks.
fn probe(url: &str) -> bool {
    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if matches!(
                handle.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::MultiThread
            ) =>
        {
            tokio::task::block_in_place(|| tcp_probe(url))
        }
        _ => tcp_probe(url),
    }
}

fn tcp_probe(url: &str) -> bool {
    let (host, port) = match split_host_port(url) {
        Some(hp) => hp,
        None => return false,
    };

    // Fast path: numeric IP literals need no DNS lookup (the deployment host
    // has no MagicDNS, so the URL must use the tailnet IP anyway).
    if let Ok(ip) = host.parse::<IpAddr>() {
        return TcpStream::connect_timeout(&SocketAddr::new(ip, port), PROBE_TIMEOUT).is_ok();
    }

    // Hostnames: resolve (this may block on DNS) and try each address.
    match (host, port).to_socket_addrs() {
        Ok(addrs) => addrs
            .into_iter()
            .any(|addr| TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).is_ok()),
        Err(_) => false,
    }
}

/// Extract `(host, port)` from a URL like
/// `http://100.64.0.1:11434/v1/chat/completions`. Defaults to port 443 for
/// `https://` URLs and 80 otherwise. IPv6 literals are not supported.
fn split_host_port(url: &str) -> Option<(&str, u16)> {
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (Some(s), r),
        None => (None, url),
    };
    let authority = rest.split('/').next().unwrap_or("");
    // Strip any userinfo.
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    if authority.is_empty() {
        return None;
    }
    let default_port: u16 = match scheme {
        Some("https") => 443,
        _ => 80,
    };
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => Some((host, port.parse::<u16>().ok()?)),
        _ => Some((authority, default_port)),
    }
}

/// Serializes tests that mutate the shared availability cache and `OLLAMA_URL`.
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

/// Test-only: force a provider's cache entry to "live" so selection tests
/// do not depend on a real endpoint being reachable.
#[cfg(test)]
pub(crate) fn force_provider_live(provider_id: &str) {
    lock_cache().insert(provider_id.to_string(), (Instant::now(), true));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_provider_is_live_without_probe() {
        // No API key, no URL configured, nothing listening — a provider with
        // no `url_env_var` must report live without any probing.
        assert!(is_provider_live("groq"));
    }

    #[test]
    fn unknown_provider_is_live() {
        // Rejected upstream by `provider_has_key`; availability must not
        // additionally block it.
        assert!(is_provider_live("no-such-provider"));
    }

    #[test]
    fn mark_provider_down_makes_provider_not_live() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1/v1/chat/completions");
        mark_provider_down("ollama");
        assert!(!is_provider_live("ollama"));
    }

    #[test]
    fn mark_provider_down_ignores_cloud_providers() {
        // Cloud providers are never probed, so marking them down is a no-op.
        mark_provider_down("groq");
        assert!(is_provider_live("groq"));
    }

    #[test]
    fn split_host_port_parses_urls() {
        assert_eq!(
            split_host_port("http://100.64.0.1:11434/v1/chat/completions"),
            Some(("100.64.0.1", 11434))
        );
        assert_eq!(split_host_port("https://example.com/v1"), Some(("example.com", 443)));
        assert_eq!(split_host_port("http://example.com"), Some(("example.com", 80)));
        assert_eq!(split_host_port(""), None);
    }
}
