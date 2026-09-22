//! Opt-in bounded backoff.
//!
//! Retries only `429` and `5xx`, and only for idempotent methods. `POST` is never
//! retried automatically: the API publishes no idempotency key, so a retried
//! create could duplicate a resource.
use crate::{Error, Method, Operation};
use std::time::Duration;

/// Exponential backoff with a ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Backoff {
    pub initial: Duration,
    pub max: Duration,
    /// Multiplier applied per attempt, as a percentage: 200 doubles.
    pub factor_percent: u32,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(500),
            max: Duration::from_secs(30),
            factor_percent: 200,
        }
    }
}

impl Backoff {
    /// Delay before attempt `attempt`, counting the first retry as 0.
    pub fn delay(&self, attempt: u32) -> Duration {
        let mut delay = self.initial;
        for _ in 0..attempt {
            delay = delay.saturating_mul(self.factor_percent) / 100;
            if delay >= self.max {
                return self.max;
            }
        }
        delay.min(self.max)
    }
}

/// How many times to retry, and how long to wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    /// Retries after the first attempt. `0` disables retrying.
    pub max_retries: u32,
    pub backoff: Backoff,
    /// Honour a `429`'s `X-RateLimit-Reset` when it is in the future and nearer
    /// than the backoff ceiling.
    pub respect_reset: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff: Backoff::default(),
            respect_reset: true,
        }
    }
}

impl RetryPolicy {
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// The delay to wait before retrying, or `None` to give up.
    ///
    /// `now_unix` lets a caller supply the current time; the crate reads no clock
    /// of its own when it is `None`, and then ignores `respect_reset`.
    pub fn delay_for(
        &self,
        error: &Error,
        method: Method,
        attempt: u32,
        now_unix: Option<u64>,
    ) -> Option<Duration> {
        if attempt >= self.max_retries || !error.is_retryable() || !method.is_idempotent() {
            return None;
        }
        let backoff = self.backoff.delay(attempt);
        if self.respect_reset
            && let Error::RateLimited(limit) = error
            && let (Some(reset), Some(now)) = (limit.reset, now_unix)
            && reset > now
        {
            let wait = Duration::from_secs(reset - now);
            return Some(wait.min(self.backoff.max).max(backoff));
        }
        Some(backoff)
    }
}

fn now_unix() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs())
}

impl crate::Client {
    /// Send with bounded retries. See [`RetryPolicy`] for what is retried.
    pub async fn send_retrying<O: Operation + Clone>(
        &self,
        op: O,
        policy: RetryPolicy,
    ) -> Result<O::Output, Error> {
        let method = op.request()?.method;
        let mut attempt = 0;
        loop {
            match self.send(op.clone()).await {
                Ok(output) => return Ok(output),
                Err(error) => match policy.delay_for(&error, method, attempt, now_unix()) {
                    Some(delay) => {
                        // Never block the executor: the async client sleeps on
                        // the timer, not the thread.
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    }
                    None => return Err(error),
                },
            }
        }
    }
}

#[cfg(feature = "blocking")]
impl crate::blocking::Client {
    /// Send with bounded retries. See [`RetryPolicy`] for what is retried.
    pub fn send_retrying<O: Operation + Clone>(
        &self,
        op: O,
        policy: RetryPolicy,
    ) -> Result<O::Output, Error> {
        let method = op.request()?.method;
        let mut attempt = 0;
        loop {
            match self.send(op.clone()) {
                Ok(output) => return Ok(output),
                Err(error) => match policy.delay_for(&error, method, attempt, now_unix()) {
                    Some(delay) => {
                        std::thread::sleep(delay);
                        attempt += 1;
                    }
                    None => return Err(error),
                },
            }
        }
    }
}
