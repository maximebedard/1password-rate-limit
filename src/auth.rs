use std::fmt;

use axum::{
  extract::{State, TypedHeader},
  headers::{authorization::Bearer, Authorization},
  http::{Request, StatusCode},
  middleware::Next,
  response::IntoResponse,
  RequestPartsExt,
};

use crate::SharedAppState;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApiToken(pub String);

impl fmt::Display for ApiToken {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

pub async fn token_auth<B>(State(state): State<SharedAppState>, req: Request<B>, next: Next<B>) -> impl IntoResponse {
  let (mut parts, body) = req.into_parts();

  let api_token = parts
    .extract::<TypedHeader<Authorization<Bearer>>>()
    .await
    .map(|header| ApiToken(header.token().to_string()))
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

  // Check in the hashmap in the shared state if the key is present. Act as a very simple authentication method.
  let is_authenticated = state.read().expect("rwlock is poised").vaults.contains_key(&api_token);

  if !is_authenticated {
    return Err(StatusCode::UNAUTHORIZED);
  }

  // println!("authenticated api key `{}`", api_token);

  let mut req = Request::from_parts(parts, body);
  req.extensions_mut().insert(api_token);
  Ok(next.run(req).await)
}
