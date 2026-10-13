//! Per-host token-bucket rate limiter for content-fetch egress (W-ENRICH-1, doc 06 §2b).
//!
//! ## Responsibilities
//! - Enforce a per-host request budget so the content-fetch job never exceeds a host's quota. GitHub's
//!   unauthenticated REST limit is 60 req/hr/IP — "the hardest constraint" (06 §1) — so the default
//!   host budget is sized for it, and `api.github.com` gets the explicit GitHub budget.
//! - Keep the token-bucket MATH pure + unit-tested ([`TokenBucket`]), and the process-global
//!   host→bucket registry ([`acquire_host_token`]) a thin lock around it.
//!
//! - Hold a host entirely when it answers HTTP 429, for as long as it asked ([`hold_host`]), so no
//!   queued fetch for that host retries before then.
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
use std::time::{Duration, Instant};

/// How long a host is left alone after a 429 that does not say for how long.
pub(crate) const DEFAULT_HOST_HOLD: Duration = Duration::from_secs(60 * 60);
/// Bounds on the wait a host asks for, so a bad header neither hammers it nor parks it for weeks.
const MIN_HOST_HOLD: Duration = Duration::from_secs(60);
const MAX_HOST_HOLD: Duration = Duration::from_secs(24 * 60 * 60);

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
    /// The host asked for no requests before this (HTTP 429).
    held_until: Option<Instant>,
}

impl TokenBucket {
    /// Creates a full bucket (starts at capacity so the first burst is allowed immediately).
    pub(crate) fn new(budget: HostBudget, now: Instant) -> Self {
        Self { budget, tokens: budget.capacity, last_refill: now, held_until: None }
    }

    /// Refuses every request until `until`; a later hold extends an earlier one.
    fn hold(&mut self, until: Instant) {
        self.held_until = Some(self.held_until.map_or(until, |held| held.max(until)));
    }

    fn held_for(&self, now: Instant) -> Option<Duration> {
        self.held_until.filter(|until| now < *until).map(|until| until - now)
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
        if self.held_for(now).is_some() {
            return false;
        }
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
    /// deferred-job ETA is rounded UP to whole seconds so a job is never re-scheduled a hair
    /// before a token is actually available (which would burn another empty drain). At GitHub's 60/hr
    /// refill this is ≈ 60s when the bucket is empty.
    pub(crate) fn next_token_eta_secs(&mut self, now: Instant) -> u64 {
        if let Some(held) = self.held_for(now) {
            return held.as_secs_f64().ceil() as u64;
        }
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

/// Seconds until `host` would grant the next token (the requeue ETA for a throttled job).
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

/// Sends no request to `host` for `wait` (clamped to a minute .. a day): it answered HTTP 429.
/// Every queued fetch for the host waits, not only the one that was refused, and the deferred job
/// reads its new time from [`next_token_eta_secs`].
pub(crate) fn hold_host(host: &str, wait: Duration) {
    let host_key = host.trim().to_ascii_lowercase();
    if host_key.is_empty() {
        return;
    }
    let budget = budget_for_host(&host_key);
    let now = Instant::now();
    let registry = HOST_BUCKETS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut buckets = registry.lock().expect("host rate-limit registry lock");
    let bucket = buckets.entry(host_key).or_insert_with(|| TokenBucket::new(budget, now));
    bucket.hold(now + wait.clamp(MIN_HOST_HOLD, MAX_HOST_HOLD));
}
