//! SSRF guard for outbound link-preview / og:image fetches.
//!
//! ## Responsibilities
//! - Classify a fetch target's host as public vs. non-public (loopback,
//!   RFC1918 private, link-local incl. the `169.254.169.254` cloud-metadata
//!   endpoint, CGNAT, IPv6 unique-local / link-local, multicast, reserved).
//! - Expose a single `url_target_is_blocked` predicate the og:image pipeline
//!   calls before issuing a GET to a page URL or an attacker-supplied
//!   `og:image` URL.
//!
//! ## Not responsible for
//! - HTTP(S) scheme enforcement at the page boundary (the production entry
//!   already requires `https://`); this module independently rejects any
//!   non-http(s) scheme it is asked to validate.
//! - DNS-rebinding-proof connection pinning, and re-validating every redirect
//!   hop. reqwest follows redirects with its own resolver, so a page that
//!   resolves public but 30x-redirects through a private host is only a
//!   partial residual (tracked in BACKLOG). The dominant vectors — a page URL
//!   or scraped `og:image` URL pointing *directly* at a private host — are
//!   fully closed here.
//!
//! ## Why this exists
//! `og:image` URLs are scraped from attacker-controlled page HTML and page URLs
//! come from imported history. Without destination filtering a single visited
//! page could make PathKeep GET internal services (router admin pages, Docker
//! APIs, cloud-metadata), which both leaks that those services exist and
//! violates the data-sovereignty principle (no traffic the user didn't ask
//! for). Resolution *failure* fails open: an unresolvable host cannot reach an
//! internal service, so blocking it would only break the legitimate retry
//! cadence without adding safety.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// True when `ip` is anything other than a routable public address.
pub(crate) fn ip_is_non_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_non_public(v4),
        // An IPv4-mapped v6 address (`::ffff:127.0.0.1`) must be judged by its
        // embedded v4 address. Use `to_ipv4_mapped` (NOT `to_ipv4`, which also
        // maps v4-*compatible* addresses like `::1` → `0.0.0.1` and would let
        // the v6 loopback slip past the v4 loopback check).
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(mapped) => ipv4_is_non_public(mapped),
            None => ipv6_is_non_public(v6),
        },
    }
}

fn ipv4_is_non_public(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        // CGNAT / RFC6598 shared address space 100.64.0.0/10.
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        // IETF protocol assignments 192.0.0.0/24.
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        // Reserved 240.0.0.0/4 (includes the 255.x broadcast block).
        || octets[0] >= 240
}

fn ipv6_is_non_public(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        // Unique local addresses fc00::/7.
        || (segments[0] & 0xfe00) == 0xfc00
        // Link-local unicast fe80::/10.
        || (segments[0] & 0xffc0) == 0xfe80
}

/// True when PathKeep must refuse to fetch `url`: it is unparseable, uses a
/// non-http(s) scheme, or its host is a literal / resolves to a non-public IP.
///
/// Resolution failure returns `false` (fail open) on purpose — see the module
/// header. A literal IP host is classified without any DNS lookup.
pub(crate) fn url_target_is_blocked(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return true;
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return true;
    }
    let host = parsed.host_str().unwrap_or("");
    // `host_str` keeps IPv6 brackets (`[::1]`); strip them before parsing.
    let host_for_ip =
        host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
    if let Ok(ip) = host_for_ip.parse::<IpAddr>() {
        return ip_is_non_public(ip);
    }
    let port = parsed.port_or_known_default().unwrap_or(443);
    match (host, port).to_socket_addrs() {
        // Block if ANY resolved address is non-public so a host that returns
        // one public and one private record can't smuggle a request through.
        Ok(addresses) => addresses.into_iter().any(|address| ip_is_non_public(address.ip())),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_non_public_ranges_are_rejected_and_public_is_allowed() {
        for blocked in [
            "0.0.0.0",
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254", // cloud metadata
            "255.255.255.255", // broadcast
            "192.0.2.1",       // documentation TEST-NET-1
            "198.51.100.1",    // documentation TEST-NET-2
            "203.0.113.1",     // documentation TEST-NET-3
            "224.0.0.1",       // multicast
            "100.64.0.1",      // CGNAT
            "192.0.0.1",       // IETF protocol assignments
            "240.0.0.1",       // reserved
        ] {
            let ip: IpAddr = blocked.parse().unwrap();
            assert!(ip_is_non_public(ip), "{blocked} should be non-public");
        }
        for public in ["8.8.8.8", "1.1.1.1", "93.184.216.34"] {
            let ip: IpAddr = public.parse().unwrap();
            assert!(!ip_is_non_public(ip), "{public} should be public");
        }
    }

    #[test]
    fn ipv6_non_public_ranges_are_rejected_and_public_is_allowed() {
        for blocked in [
            "::",                // unspecified
            "::1",               // loopback
            "ff02::1",           // multicast
            "fc00::1",           // unique local
            "fd12:3456:789a::1", // unique local
            "fe80::1",           // link-local
            "::ffff:127.0.0.1",  // v4-mapped loopback
            "::ffff:10.0.0.1",   // v4-mapped private
        ] {
            let ip: IpAddr = blocked.parse().unwrap();
            assert!(ip_is_non_public(ip), "{blocked} should be non-public");
        }
        for public in ["2606:4700:4700::1111", "::ffff:8.8.8.8"] {
            let ip: IpAddr = public.parse().unwrap();
            assert!(!ip_is_non_public(ip), "{public} should be public");
        }
    }

    #[test]
    fn url_target_is_blocked_covers_scheme_literal_and_resolution_paths() {
        // Unparseable / non-http(s) schemes are refused.
        assert!(url_target_is_blocked("not a url"));
        assert!(url_target_is_blocked("ftp://example.com/resource"));
        assert!(url_target_is_blocked("file:///etc/passwd"));

        // Literal IP hosts are classified without DNS.
        assert!(url_target_is_blocked("https://127.0.0.1/x"));
        assert!(url_target_is_blocked("http://169.254.169.254/latest/meta-data/"));
        assert!(url_target_is_blocked("https://[::1]:8443/")); // bracketed IPv6
        assert!(!url_target_is_blocked("https://8.8.8.8/x"));

        // `localhost` resolves to loopback via the hosts file (no network).
        assert!(url_target_is_blocked("http://localhost:9000/admin"));

        // An unresolvable host fails open: it cannot reach an internal service.
        assert!(!url_target_is_blocked("https://og-image-test.invalid./"));
    }
}
