# 1password interview - Rate limiter

A small API server with an in-memory rate limiter.

# Running the application

The application is built using rust 1.71 and uses Axum & Tokio.

```sh
cargo run
```

# Request samples

The application uses a Bearer token to log in and has 2 hardcoded accounts for demonstration purposes: `abc:` and `def:`.

```
# max_rpm = 3
curl -v -H "Authorization: Bearer abc"  -X POST http://localhost:3000/vault --data '{}'

# max_rpm = 1200
curl -v -H "Authorization: Bearer abc"  -X GET http://localhost:3000/vault/items

# max_rpm = 60
curl -v -H "Authorization: Bearer abc"  -X PUT http://localhost:3000/vault/items/123 --data '{}'
```

# Simple test cases

```sh
for i in $(seq 1 3);
do
  curl -H "Authorization: Bearer abc" -X POST http://localhost:3000/vault --data '{}' --fail --silent --output /dev/null --show-error --fail
  sleep 20
done
```

```sh
for i in $(seq 1 2);
do
  for j in $(seq 1 3); do
    curl -H "Authorization: Bearer abc" -X POST http://localhost:3000/vault --data '{}' --fail --silent --output /dev/null --show-error --fail
  done
  sleep 60
done
```

```sh
for i in $(seq 1 4);
do
  curl -H "Authorization: Bearer abc" -X POST http://localhost:3000/vault --data '{}' --fail --silent --output /dev/null --show-error --fail
done
```

```sh
for i in $(seq 1 3);
do
  curl -H "Authorization: Bearer abc" -X POST http://localhost:3000/vault --data '{}' --fail --silent --output /dev/null --show-error --fail
  curl -H "Authorization: Bearer def" -X POST http://localhost:3000/vault --data '{}' --fail --silent --output /dev/null --show-error --fail
  sleep 20
done
```

```sh
for i in $(seq 1 1201);
do
  curl -H "Authorization: Bearer abc" -X GET http://localhost:3000/vault/items --data '{}' --fail --silent --output /dev/null --show-error --fail
done
```
