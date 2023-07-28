use std::{
  collections::HashMap,
  sync::{Arc, RwLock},
};

use auth::ApiToken;
use axum::{
  extract::Path,
  middleware,
  response::IntoResponse,
  routing::{get, post, put},
  Router,
};

mod auth;
mod rate_limit;

pub struct Vault;

pub struct AppState {
  // I'm using a shared state to store the api tokens to validate that the authentication works.
  vaults: HashMap<ApiToken, Vault>,
}

type SharedAppState = Arc<RwLock<AppState>>;

#[tokio::main]
async fn main() {
  // Add a list of static entries.
  let mut vaults = HashMap::new();
  vaults.insert(ApiToken("abc".to_string()), Vault);
  vaults.insert(ApiToken("def".to_string()), Vault);
  let state = Arc::new(RwLock::new(AppState { vaults }));

  // Setup a router with an authentication middleware that covers all routes.
  // Each of the /vault* routes register the RateLimit middleware with the custom max_rpm they can handle.
  let app = Router::new()
    .route(
      "/vault",
      post(create_vault).route_layer(rate_limit::RateLimitLayer { max_rpm: 3 }),
    )
    .route(
      "/vault/items",
      get(list_vault_items).route_layer(rate_limit::RateLimitLayer { max_rpm: 1200 }),
    )
    .route(
      "/vault/items/:id",
      put(create_vault_item).route_layer(rate_limit::RateLimitLayer { max_rpm: 60 }),
    )
    .route_layer(middleware::from_fn_with_state(state.clone(), auth::token_auth))
    .with_state(state);

  println!("listening on [::]:3000");
  axum::Server::bind(&"[::]:3000".parse().expect("failed to parse socket address"))
    .serve(app.into_make_service())
    .await
    .expect("failed to start server");
}

async fn create_vault() -> impl IntoResponse {
  println!("create_vault => 200 OK");
  // Return 200 OK
  ()
}

async fn list_vault_items() -> impl IntoResponse {
  println!("list_vault_items => 200 OK");
  // Return 200 OK
  ()
}

async fn create_vault_item(Path(_id): Path<u64>) -> impl IntoResponse {
  println!("create_vault_item => 200 OK");
  // Return 200 OK
  ()
}
