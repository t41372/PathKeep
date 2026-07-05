//! Per-host token-bucket rate limiter for content-fetch egress (W-ENRICH-1, doc 06 §2b).
//!
//! ## Responsibilities
//! - Enforce a per-host request budget so the content-fetch job never exceeds a host's quota. GitHub's
//!   unauthenticated REST limit is 60 req/hr/IP — "the hardest constraint" (06 §1) — so the default
//!   host budget is sized for it, and `api.github.com` gets the explicit GitHub budget.
//! - Keep the token-bucket MATH pure + unit-tested ([`TokenBucket`]), and the process-global
//!   host→bucket registry ([`acquire_host_token`]) a thin lock around it.
//!
//! ## Not responsible for
//! - Fetching (the runner does), the SSRF guard, or the negative-cache cadence (that is the per-row
//!   `refetch_after`). This is purely the "may I make ANOTHER request to this host right now" gate.
//!
//! ## Why a token bucket (not a fixed window)
//! A token bucket refills continuously, so it both caps the long-run rate (60/hr) AND smooths bursts
//! (a small burst capacity lets the runner fetch a few queued GitHub repos back-to-back, then throttle)
//! — without the thundering-herd edge a fixed hourly window has at the window boundary.
//!
//! ## Pluggability (user-PAT future)
//! The budget is resolved per host via [`budget_for_host`]; a future user-PAT path (5000/hr) swaps
//! GitHub's budget by passing a different [`HostBudget`] — the bucket math is unchanged. Left as a
//! seam (06 §1: "leave the rate-limiter pluggable").

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// A per-host request budget: a refill rate (tokens/sec) and a burst capacity (max tokens).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HostBudget {
    /// Tokens added per second. 60/hr ⇒ 60 / 3600 ≈ 0.01667 tok/s.
    pub refill_per_sec: f64,
    /// Maximum tokens the bucket holds (burst capacity).
    pub capacity: f64,
}

impl HostBudget {
    /// Builds a budget from a "requests per hour" quota with a small burst allowance.
    ///
    /// `const fn` so the production budgets below are defined through it (one place expresses the
    /// req/hr → tok/s conversion), and so a future user-PAT path (5000/hr) can swap a budget without
    /// re-deriving the math. A zero burst is floored to 1 so a bucket can always hold at least a token.
    pub(crate) const fn per_hour(requests_per_hour: f64, burst: f64) -> Self {
        let capacity = if burst < 1.0 { 1.0 } else { burst };
        Self { refill_per_sec: requests_per_hour / 3600.0, capacity }
    }
}

/// GitHub unauthenticated REST limit: 60 req/hr/IP, burst of 5 (06 §1, the hardest constraint).
pub(crate) const GITHUB_HOST_BUDGET: HostBudget = HostBudget::per_hour(60.0, 5.0);

/// Default budget for any other host: a conservative ~120 req/hr with a burst of 8.
///
/// Generic content pages have no published quota; a polite default keeps PathKeep from hammering a
/// blog/news host while still letting a small queue drain. LOW concurrency (the worker uses a single
/// enrichment lane) plus this budget keeps egress well-mannered (06 §5).
pub(crate) const DEFAULT_HOST_BUDGET: HostBudget = HostBudget::per_hour(120.0, 8.0);

/// Resolves the [`HostBudget`] for a host (lower-cased registrable-ish host string).
///
/// `api.github.com` (and `github.com`) get the strict GitHub budget; everything else gets the polite
/// default. PURE → unit-tested. A future user-PAT path overrides GitHub's budget here.
pub(crate) fn budget_for_host(host: &str) -> HostBudget {
    let host = host.trim().to_ascii_lowercase();
    let host_norm = host.strip_prefix("www.").unwrap_or(&host);
    if host_norm == "api.github.com" || host_norm == "github.com" {
        GITHUB_HOST_BUDGET
    } else {
        DEFAULT_HOST_BUDGET
    }
}

/// A continuous-refill token bucket. Pure (time is injected) so the math is unit-tested.
#[derive(Debug, Clone)]
pub(crate) struct TokenBucket {
    budget: HostBudget,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    /// Creates a full bucket (starts at capacity so the first burst is allowed immediately).
    pub(crate) fn new(budget: HostBudget, now: Instant) -> Self {
        Self { budget, tokens: budget.capacity, last_refill: now }
    }

    /// Refills the bucket for the elapsed time since the last refill, clamped to capacity.
    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill).as_secs_f64();
        if elapsed > 0.0 {
            self.tokens =
                (self.tokens + elapsed * self.budget.refill_per_sec).min(self.budget.capacity);
            self.last_refill = now;
        }
    }

    /// Tries to take one token at `now`. Returns true (and decrements) when a token was available.
    ///
    /// PURE (time injected): refills for elapsed time, then consumes one token if ≥ 1 is available.
    /// The bucket starts full, so the first `capacity` calls in a burst succeed, after which calls
    /// succeed at the refill rate — exactly the 60/hr long-run cap with a small burst (06 §2b).
    pub(crate) fn try_take(&mut self, now: Instant) -> bool {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Seconds until the bucket holds one whole token again (0 when one is already available).
    ///
    /// PURE (time injected): refills for elapsed time, then returns the wait for the next token. The
    /// deferred-job ETA (SEC-2) is rounded UP to whole seconds so a job is never re-scheduled a hair
    /// before a token is actually available (which would burn another empty drain). At GitHub's 60/hr
    /// refill this is ≈ 60s when the bucket is empty.
    pub(crate) fn next_token_eta_secs(&mut self, now: Instant) -> u64 {
        self.refill(now);
        if self.tokens >= 1.0 {
            return 0;
        }
        let needed = 1.0 - self.tokens;
        // `refill_per_sec` is always > 0 for our budgets (per-hour quotas), so this never divides by
        // zero; guard defensively anyway so a future zero-refill budget yields a finite, capped wait.
        if self.budget.refill_per_sec <= 0.0 {
            return u64::from(u32::MAX);
        }
        (needed / self.budget.refill_per_sec).ceil() as u64
    }
}

/// Process-global host→bucket registry. One bucket per host, shared across the (low-concurrency)
/// enrichment worker lane.
static HOST_BUCKETS: OnceLock<Mutex<HashMap<String, TokenBucket>>> = OnceLock::new();

/// Attempts to acquire one egress token for `host` against its resolved budget.
///
/// Returns true when a request to `host` is allowed right now (and consumes a token); false when the
/// host's bucket is empty (the runner then skips the fetch and leaves the job queued for a later
/// drain, so it is NOT a failure — just back-pressure). Buckets persist for the process lifetime so
/// the budget is honoured across separate job claims. Thin lock around the pure [`TokenBucket`] math.
pub(crate) fn acquire_host_token(host: &str) -> bool {
    let host_key = host.trim().to_ascii_lowercase();
    if host_key.is_empty() {
        // An unknown host can't be rate-limited meaningfully; allow it (the SSRF guard + per-row
        // negative-cache are the real safety nets) rather than deadlocking the queue on a blank host.
        return true;
    }
    let budget = budget_for_host(&host_key);
    let now = Instant::now();
    let registry = HOST_BUCKETS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut buckets = registry.lock().expect("host rate-limit registry lock");
    let bucket = buckets.entry(host_key).or_insert_with(|| TokenBucket::new(budget, now));
    bucket.try_take(now)
}

/// Seconds until `host` would grant the next token (the requeue ETA for a throttled job, SEC-2).
///
/// Reads the SAME process-global bucket [`acquire_host_token`] consumes, so a job that just failed to
/// acquire a token can ask "when will one be free?" and re-schedule itself then (rather than being
/// terminally cancelled). Returns 0 for a blank/unbucketed host (it is never throttled) so the caller
/// never defers a job it would immediately re-run. Thin lock around the pure [`TokenBucket`] ETA math.
pub(crate) fn next_token_eta_secs(host: &str) -> u64 {
    let host_key = host.trim().to_ascii_lowercase();
    if host_key.is_empty() {
        return 0;
    }
    let budget = budget_for_host(&host_key);
    let now = Instant::now();
    let registry = HOST_BUCKETS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut buckets = registry.lock().expect("host rate-limit registry lock");
    let bucket = buckets.entry(host_key).or_insert_with(|| TokenBucket::new(budget, now));
    bucket.next_token_eta_secs(now)
}

/// Resets a host's bucket to FULL (test-only), isolating rate-limit assertions from cross-test state.
///
/// The bucket registry is process-global, so tests that fetch GitHub (which now consume the shared
/// `api.github.com` bucket, SEC-1) would race on its token count. This refills a named host's bucket so
/// each test starts from a known-full state without disabling the production limiter.
#[cfg(test)]
pub(crate) fn reset_host_bucket_for_test(host: &str) {
    let host_key = host.trim().to_ascii_lowercase();
    if host_key.is_empty() {
        return;
    }
    let budget = budget_for_host(&host_key);
    let now = Instant::now();
    let registry = HOST_BUCKETS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut buckets = registry.lock().expect("host rate-limit registry lock");
    buckets.insert(host_key, TokenBucket::new(budget, now));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn budget_for_host_uses_github_budget_for_github_hosts() {
        assert_eq!(budget_for_host("api.github.com"), GITHUB_HOST_BUDGET);
        assert_eq!(budget_for_host("github.com"), GITHUB_HOST_BUDGET);
        assert_eq!(budget_for_host("WWW.GitHub.com"), GITHUB_HOST_BUDGET);
        assert_eq!(budget_for_host("example.com"), DEFAULT_HOST_BUDGET);
    }

    #[test]
    fn per_hour_converts_quota_to_refill_rate() {
        let budget = HostBudget::per_hour(60.0, 5.0);
        assert!((budget.refill_per_sec - 60.0 / 3600.0).abs() < 1e-9);
        assert_eq!(budget.capacity, 5.0);
        // A zero burst is floored to 1 so a bucket can always hold at least one token.
        assert_eq!(HostBudget::per_hour(60.0, 0.0).capacity, 1.0);
    }

    #[test]
    fn token_bucket_allows_burst_then_throttles_until_refill() {
        let start = Instant::now();
        let mut bucket = TokenBucket::new(GITHUB_HOST_BUDGET, start);
        // Burst capacity is 5 → first 5 immediate takes succeed.
        for _ in 0..5 {
            assert!(bucket.try_take(start));
        }
        // The 6th immediate take fails (bucket drained, no time elapsed).
        assert!(!bucket.try_take(start));
        // After enough time to refill exactly one token (1 / refill_per_sec seconds), one more
        // succeeds. 60/hr ⇒ 60 s per token.
        let later = start + Duration::from_secs(61);
        assert!(bucket.try_take(later));
        // …and the one after that fails again until more time passes.
        assert!(!bucket.try_take(later));
    }

    #[test]
    fn token_bucket_refill_is_clamped_to_capacity() {
        let start = Instant::now();
        let mut bucket = TokenBucket::new(DEFAULT_HOST_BUDGET, start);
        // Drain it.
        for _ in 0..8 {
            assert!(bucket.try_take(start));
        }
        assert!(!bucket.try_take(start));
        // Wait a very long time — the bucket refills only up to capacity (8), not unbounded.
        let much_later = start + Duration::from_secs(100 * 3600);
        for _ in 0..8 {
            assert!(bucket.try_take(much_later));
        }
        assert!(!bucket.try_take(much_later));
    }

    #[test]
    fn acquire_host_token_allows_blank_host() {
        // A blank host cannot be bucketed; it is allowed so the queue never deadlocks on it.
        assert!(acquire_host_token(""));
        assert!(acquire_host_token("   "));
    }

    #[test]
    fn acquire_host_token_consumes_from_a_real_bucket() {
        // Use a unique host so the process-global registry state is isolated from other tests.
        let host = format!("rate-limit-test-{}.example", std::process::id());
        // Default burst is 8; the first 8 succeed, the 9th (no time elapsed in a tight loop) may
        // fail. We only assert the first acquire succeeds — the math itself is covered purely above.
        assert!(acquire_host_token(&host));
    }

    #[test]
    fn token_bucket_eta_is_zero_when_full_and_positive_when_drained() {
        let start = Instant::now();
        let mut bucket = TokenBucket::new(GITHUB_HOST_BUDGET, start);
        // A full bucket has a token now → 0s ETA.
        assert_eq!(bucket.next_token_eta_secs(start), 0);
        // Drain it; the next token is ~60s away (60/hr refill, rounded up).
        for _ in 0..5 {
            assert!(bucket.try_take(start));
        }
        let eta = bucket.next_token_eta_secs(start);
        assert!((55..=61).contains(&eta), "GitHub refill ETA should be ~60s, got {eta}");
    }

    #[test]
    fn token_bucket_eta_is_capped_for_a_zero_refill_budget() {
        // A degenerate zero-refill budget never refills, so a drained bucket yields the capped (finite)
        // wait rather than dividing by zero.
        let zero = HostBudget { refill_per_sec: 0.0, capacity: 1.0 };
        let start = Instant::now();
        let mut bucket = TokenBucket::new(zero, start);
        assert!(bucket.try_take(start));
        assert_eq!(bucket.next_token_eta_secs(start), u64::from(u32::MAX));
    }

    #[test]
    fn next_token_eta_secs_registry_reports_blank_and_drained_hosts() {
        // A blank host is never throttled → 0 ETA.
        assert_eq!(next_token_eta_secs(""), 0);
        // A fresh unique host starts full → 0; after draining its burst it reports a positive ETA.
        let host = format!("eta-registry-{}.example", std::process::id());
        reset_host_bucket_for_test(&host);
        assert_eq!(next_token_eta_secs(&host), 0);
        while acquire_host_token(&host) {}
        assert!(next_token_eta_secs(&host) > 0);
    }

    #[test]
    fn reset_host_bucket_for_test_refills_and_ignores_blank() {
        let host = format!("reset-{}.example", std::process::id());
        while acquire_host_token(&host) {}
        assert!(!acquire_host_token(&host), "bucket drained");
        reset_host_bucket_for_test(&host);
        assert!(acquire_host_token(&host), "reset refills the bucket");
        // A blank host is a no-op (does not panic or insert a bucket).
        reset_host_bucket_for_test("   ");
    }
}
