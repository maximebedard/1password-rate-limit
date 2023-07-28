use axum::{
  body::Body,
  http::{Request, StatusCode},
  response::{IntoResponse, Response},
};
use std::{
  collections::HashMap,
  future::Future,
  pin::Pin,
  sync::{Arc, Mutex},
  task::{Context, Poll},
  time::{Duration, Instant},
};
use tower::{Layer, Service};

use crate::auth::ApiToken;

const TTL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct RateLimitLayer {
  pub max_rpm: u32,
}

impl<S> Layer<S> for RateLimitLayer {
  type Service = RateLimitMiddleware<S>;

  fn layer(&self, inner: S) -> Self::Service {
    RateLimitMiddleware {
      inner,
      buckets: Arc::new(Mutex::new(HashMap::new())),
      max_rpm: self.max_rpm,
    }
  }
}

#[derive(Clone)]
pub struct RateLimitMiddleware<S> {
  inner: S,
  buckets: Arc<Mutex<HashMap<ApiToken, (Instant, u32)>>>,
  max_rpm: u32,
}

impl<S> Service<Request<Body>> for RateLimitMiddleware<S>
where
  S: Service<Request<Body>, Response = Response> + Send + 'static,
  S::Future: Send + 'static,
{
  type Response = S::Response;
  type Error = S::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

  fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    self.inner.poll_ready(cx)
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    let api_token = request
      .extensions()
      .get::<ApiToken>()
      .expect("rate limit must be used in combination with the token auth middleware")
      .clone();

    // TODO: add garbage collection for expired values to prevent leaking memory.
    // A naive implementation of this garbage collection routine could be to spawn
    // a task that scans through the entries and remove all the entries where i.elapsed() > TTL.
    // Another implementation to avoid scans could be to enqueue a task that sleeps for the duration and delete
    // the entry after the TTL.
    let (i, n) = *self
      .buckets
      .lock()
      .expect("mutex is poised")
      .entry(api_token.clone())
      .and_modify(|(i, n)| {
        if i.elapsed() > TTL {
          *i = Instant::now();
          *n = 1;
        } else if *n <= self.max_rpm {
          *n += 1;
        }
      })
      .or_insert_with(|| (Instant::now(), 1));

    let future = self.inner.call(request);
    let is_rate_limited = n > self.max_rpm;
    Box::pin(async move {
      if is_rate_limited {
        let retry_after_secs = TTL.as_secs().checked_sub(i.elapsed().as_secs()).unwrap_or(0);

        println!("{} is rate limited for {}s", api_token, retry_after_secs);

        let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
        response.headers_mut().append("Retry-After", retry_after_secs.into());
        return Ok(response);
      }
      future.await
    })
  }
}
