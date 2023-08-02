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

    let (i, n) = self.incr(&api_token);
    let max_rpm = self.max_rpm;

    let future = self.inner.call(request);

    Box::pin(async move {
      if n > max_rpm {
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

impl<S> RateLimitMiddleware<S> {
  fn incr(&self, api_token: &ApiToken) -> (Instant, u32) {
    let mut buckets = self.buckets.lock().expect("mutex is poised");
    match buckets.get_mut(api_token) {
      Some((i, n)) if *n <= self.max_rpm => {
        *n += 1;
        (*i, *n)
      }
      Some(v) => *v,
      None => {
        let i = Instant::now();
        let n = 1_u32;
        self.expire(api_token);
        buckets.insert(api_token.clone(), (i.clone(), n.clone()));
        (i, n)
      }
    }
  }

  fn expire(&self, api_token: &ApiToken) {
    let buckets = self.buckets.clone();
    let api_token = api_token.clone();
    tokio::task::spawn(async move {
      tokio::time::sleep(TTL).await;
      buckets.lock().expect("mutex is poised").remove(&api_token);
    });
  }
}
