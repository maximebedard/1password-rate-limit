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
  type Future = RateLimitFuture<S::Future>;

  fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    self.inner.poll_ready(cx).map_err(Into::into)
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    let api_token = request
      .extensions()
      .get::<ApiToken>()
      .expect("rate limit must be used in combination with the token auth middleware")
      .clone();

    let (i, n) = self.decrement(&api_token);

    if n == 0 {
      return RateLimitFuture::RateLimited(i);
    }

    return RateLimitFuture::Next(self.inner.call(request));
  }
}

impl<S> RateLimitMiddleware<S> {
  fn decrement(&self, api_token: &ApiToken) -> (Instant, u32) {
    let mut buckets = self.buckets.lock().expect("mutex is poised");
    match buckets.get_mut(api_token) {
      Some((i, n)) => {
        *n = n.saturating_sub(1);
        (*i, *n)
      }
      None => {
        let i = Instant::now();
        let n = self.max_rpm;
        self.expire(api_token);
        buckets.insert(api_token.clone(), (i, n));
        (i, n)
      }
    }
  }

  fn expire(&self, api_token: &ApiToken) {
    let buckets = self.buckets.clone();
    let api_token = api_token.clone();
    tokio::task::spawn(async move {
      tokio::select! {
        _ = tokio::signal::ctrl_c() => return,
        _ = tokio::time::sleep(TTL) => {
          buckets.lock().expect("mutex is poised").remove(&api_token);
        },
      }
    });
  }
}

#[pin_project::pin_project(project=RateLimitFutureProjection)]
pub enum RateLimitFuture<Fut> {
  RateLimited(Instant),
  Next(#[pin] Fut),
}

impl<F, E> Future for RateLimitFuture<F>
where
  F: Future<Output = Result<Response, E>>,
{
  type Output = Result<Response, E>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match self.project() {
      RateLimitFutureProjection::RateLimited(i) => {
        let retry_after_secs = TTL.as_secs().saturating_sub(i.elapsed().as_secs());
        let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
        response.headers_mut().append("Retry-After", retry_after_secs.into());
        Poll::Ready(Ok(response.into()))
      }
      RateLimitFutureProjection::Next(next) => next.poll(cx).map_err(Into::into),
    }
  }
}
