use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone, Debug)]
pub enum Backoff {
    Exponential { base: Duration },
}

impl Backoff {
    pub fn exponential(base: Duration) -> Self {
        Backoff::Exponential { base }
    }

    fn delay_at(&self, attempt: u32) -> Duration {
        match self {
            Backoff::Exponential { base } => {
                let exp = std::cmp::min(attempt.saturating_sub(1), 20);
                let factor = 1u32 << exp;
                base.saturating_mul(factor)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff: Backoff,
    pub retryable_errors: Vec<String>,
}

fn is_retryable<E: std::fmt::Display>(err: &E, retryable: &[String]) -> bool {
    let s = err.to_string();
    retryable.iter().any(|code| s.contains(code))
}

pub async fn retry_async<F, Fut, T, E>(config: RetryConfig, mut op: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if attempt >= config.max_attempts || !is_retryable(&e, &config.retryable_errors) {
                    return Err(e);
                }
                let delay = config.backoff.delay_at(attempt);
                sleep(delay).await;
            }
        }
    }
}
