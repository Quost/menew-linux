//! Currency conversion (port of the "Moedas" part of `Calculator.swift`).
//!
//! The rate comes from the European Central Bank through `api.frankfurter.app`, the same provider
//! the Mac app uses, and is fetched ONLY when the user presses Enter on a conversion
//! (`fetch_rate`). Searching never touches the network: [`query`] and [`cached_rate`] are pure.

use crate::conversions::split_conversion;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const CURRENCIES: &[&str] = &[
    "usd", "eur", "brl", "gbp", "jpy", "cny", "cad", "aud", "chf", "ars", "mxn", "clp", "cop", "inr", "krw", "sek", "nok",
    "dkk", "pln", "try", "zar", "nzd", "hkd", "sgd",
];

/// How long a fetched rate is reused (the Mac uses 6 hours).
const CACHE_TTL: Duration = Duration::from_secs(6 * 3600);

/// "100 usd em brl" -> (100, "USD", "BRL")
pub fn query(q: &str) -> Option<(f64, String, String)> {
    let (v, from, to) = split_conversion(&q.to_lowercase())?;
    if !CURRENCIES.contains(&from.as_str()) || !CURRENCIES.contains(&to.as_str()) || from == to {
        return None;
    }
    Some((v, from.to_uppercase(), to.to_uppercase()))
}

static CACHE: Mutex<Option<HashMap<String, (Instant, f64)>>> = Mutex::new(None);

pub fn cached_rate(from: &str, to: &str) -> Option<f64> {
    let guard = CACHE.lock().ok()?;
    let (at, rate) = guard.as_ref()?.get(&format!("{from}{to}"))?;
    (at.elapsed() < CACHE_TTL).then_some(*rate)
}

fn store_rate(from: &str, to: &str, rate: f64) {
    if let Ok(mut g) = CACHE.lock() {
        g.get_or_insert_with(HashMap::new).insert(format!("{from}{to}"), (Instant::now(), rate));
    }
}

/// How many HTTP requests this process has made (tests and the daemon log use it to show that
/// nothing goes out while typing).
pub static HTTP_REQUESTS: AtomicUsize = AtomicUsize::new(0);

pub fn rate_url(from: &str, to: &str) -> String {
    format!("https://api.frankfurter.app/latest?from={from}&to={to}")
}

/// Read the rate out of the provider's JSON: `{"rates": {"BRL": 5.43}}`.
pub fn parse_rate(body: &str, to: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("rates")?.get(to)?.as_f64().filter(|r| r.is_finite() && *r > 0.0)
}

/// The real request, by `curl` (no HTTP/TLS crate in the tree): one GET to a public endpoint with
/// no personal data. Only the currency codes and the amount-free rate query leave the machine.
pub fn http_get(url: &str) -> Option<String> {
    HTTP_REQUESTS.fetch_add(1, Ordering::SeqCst);
    let out = std::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--max-time",
            "10",
            // frankfurter.app answers with a redirect to its new host (api.frankfurter.dev); the Mac's
            // URLSession follows it too. Only https, at most 3 hops.
            "--location",
            "--max-redirs",
            "3",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            url,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// Fetch the rate with `get` (the real [`http_get`] in the app, a stub in tests), cache it and return it.
pub fn fetch_rate_with(get: &dyn Fn(&str) -> Option<String>, from: &str, to: &str) -> Option<f64> {
    let body = get(&rate_url(from, to))?;
    let rate = parse_rate(&body, to)?;
    store_rate(from, to, rate);
    Some(rate)
}

/// What happens on Enter: the only place where the network is used.
pub fn fetch_rate(from: &str, to: &str) -> Option<f64> {
    fetch_rate_with(&http_get, from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn recognises_currency_queries() {
        assert_eq!(query("100 usd em brl"), Some((100.0, "USD".into(), "BRL".into())));
        assert_eq!(query("100 usd to brl"), Some((100.0, "USD".into(), "BRL".into())));
        assert_eq!(query("2,5 eur para gbp"), Some((2.5, "EUR".into(), "GBP".into())));
        assert_eq!(query("100usd to brl"), Some((100.0, "USD".into(), "BRL".into())));
        // same currency, unknown code or a unit conversion are not currency queries
        assert_eq!(query("100 usd to usd"), None);
        assert_eq!(query("5 km to mi"), None);
        assert_eq!(query("100 usd to xyz"), None);
    }

    #[test]
    fn parses_the_provider_json() {
        assert_eq!(parse_rate(r#"{"amount":1.0,"base":"USD","date":"2026-11-13","rates":{"BRL":5.43}}"#, "BRL"), Some(5.43));
        assert_eq!(parse_rate("not json", "BRL"), None);
        assert_eq!(parse_rate(r#"{"rates":{}}"#, "BRL"), None);
    }

    /// Makes one real request to the public endpoint (no personal data). Run by hand:
    /// `cargo test -p menew-core real_rate -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_rate_from_the_provider() {
        let before = HTTP_REQUESTS.load(Ordering::SeqCst);
        let rate = fetch_rate("USD", "BRL");
        println!("real rate USD->BRL = {:?}, http requests made: {}", rate, HTTP_REQUESTS.load(Ordering::SeqCst) - before);
        assert!(rate.is_some_and(|r| r > 0.5 && r < 50.0));
        assert_eq!(cached_rate("USD", "BRL"), rate);
    }

    #[test]
    fn rate_url_is_the_frankfurter_one() {
        assert_eq!(rate_url("USD", "BRL"), "https://api.frankfurter.app/latest?from=USD&to=BRL");
    }

    #[test]
    fn nothing_is_fetched_until_fetch_rate_is_called() {
        let calls = Cell::new(0);
        let stub = |url: &str| {
            calls.set(calls.get() + 1);
            assert!(url.starts_with("https://api.frankfurter.app/latest"));
            Some(r#"{"rates":{"JPY":150.5}}"#.to_string())
        };
        // typing the query: pure parsing, no request, nothing cached
        assert!(query("100 chf to jpy").is_some());
        assert_eq!(cached_rate("CHF", "JPY"), None);
        assert_eq!(calls.get(), 0);
        // Enter: exactly one request, then the rate is cached
        assert_eq!(fetch_rate_with(&stub, "CHF", "JPY"), Some(150.5));
        assert_eq!(calls.get(), 1);
        assert_eq!(cached_rate("CHF", "JPY"), Some(150.5));
        assert_eq!(calls.get(), 1);
    }
}
