# docker-env
A small Rust utility crate for reading environment variables and Docker-style
secrets in containerised applications.

This library is designed for applications running in Docker or Kubernetes
environments where secrets may be injected either as environment variables or
mounted files.

## 🔧 Features
- Simple and typed access to environment variables.
- Support for Docker-style secrets via `_FILE` suffix (e.g.,
  `DATABASE_PASSWORD_FILE`).
- Fallback to default values or panics for required variables.
- Logs helpful error messages on failure (requires `tracing`).

---

## 📦 Installation
Add to your `Cargo.toml`:

```toml
[dependencies]
docker-env = "0.1"
```

## 🚀 Usage
```rust
use docker_env::{get_env, get_env_or, get_env_or_panic};

// Read a value as a secret (from a file path in ENV)
let password: String = get_env("DATABASE_PASSWORD", true).unwrap();

// Read with a fallback default
let port: u16 = get_env_or("PORT", 8080, false);

// Panic if missing (e.g., required configuration)
let api_key: String = get_env_or_panic("API_KEY", false);
```

## 🔐 Docker Secret Support
If a variable is marked with has_secret = true, docker-env will first attempt to
read its corresponding secret file.

For example, if your app expects:

```bash
DATABASE_PASSWORD_FILE=/run/secrets/db_password
```
Then calling:

```rust
get_env::<String>("DATABASE_PASSWORD", true);
```
will load the contents of /run/secrets/db_password.