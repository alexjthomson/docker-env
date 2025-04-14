//! # docker-env
//! `docker-env` is a minimal utility crate for reading environment variables
//! and Docker-style secrets in containerised Rust applications.
//!
//! This crate is designed for use in Docker, Kubernetes, and other container
//! environments where secrets may be injected as file-based mounts rather than
//! plain environment variables.
//!
//! ## Features
//! - Typed access to environment variables (`get_env<T>`, `get_env_or`, etc.).
//! - Support for secret files via the `_FILE` suffix convention.
//! - Graceful fallbacks with optional, default, or required values.
//! - Logs errors using the `tracing` crate.
//!
//! ## Docker Secret Convention
//! When a variable has a secret counterpart (e.g., `DATABASE_PASSWORD`),
//! Docker often mounts the secret as a file and sets an accompanying
//! environment variable with the `_FILE` suffix:
//!
//! ```bash
//! DATABASE_PASSWORD_FILE=/run/secrets/db_password
//! ```
//!
//! Calling:
//!
//! ```no_run
//! let password: String = docker_env::get_env("DATABASE_PASSWORD", true).unwrap();
//! ```
//!
//! will cause `docker-env` to read the contents of the file at
//! `/run/secrets/db_password` instead of the plain `DATABASE_PASSWORD`
//! variable.
//!
//! ## Example
//!
//! ```no_run
//! use docker_env::{get_env, get_env_or, get_env_or_panic};
//!
//! let db_url: String = get_env_or_panic("DATABASE_URL", false);
//! let port: u16 = get_env_or("PORT", 8080, false);
//! let api_key: String = get_env("API_KEY", true).unwrap();
//! ```
//!
//! ## When to Use
//!
//! Use this crate when your app:
//! - Runs in a Docker or Kubernetes environment
//! - Uses file-based secrets via `/run/secrets/` or similar
//! - Requires simple, typed configuration with error logging

/// Commonly used imports for convenience.
pub mod prelude {
    pub use super::{
        get_env,
        get_env_or,
        get_env_or_default,
        get_env_or_panic,
        get_secret,
    };
}

/// Reads and returns the value of an environment variable.
#[must_use]
fn get_env_internal(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            tracing::error!("Failed to read environment variable `{name}`: Not Unicode.");
            None
        },
    }
}

/// Reads the value of a Docker secret value and sanitises the output.
/// 
/// # Notes
/// Typically, Docker secrets should only contain a single line; however, this
/// function will read and return the trimmed contents of the entire file.
fn read_secret_file(path: &str) -> std::io::Result<String> {
    let content = std::fs::read_to_string(path)?;
    Ok(content.trim().to_string())
}

/// Reads an environment variable.
/// 
/// # Docker Secrets
/// If `has_secret` is `true`, it indicates that this environment variable has a
/// Docker secret counterpart which should be prioritised over the original
/// environment variable.
/// 
/// The name of secret environment variables is the same as the original name
/// but with the `"_FILE"` suffix. This should point to the file that contains
/// the secret value.
#[must_use = "Ignoring the result may cause unexpected behavior due to missing or invalid configuration."]
pub fn get_env<T: std::str::FromStr>(
    name: &str,
    has_secret: bool,
) -> Option<T> {
    if has_secret {
        // Calculate the name of the secret variable.
        let mut secret_name = name.to_owned();
        secret_name.push_str("_FILE");

        if let Some(value) = get_secret(&secret_name) {
            return Some(value);
        }
    }

    let value = get_env_internal(name)?;
    match value.parse() {
        Ok(parsed_value) => Some(parsed_value),
        Err(_) => {
            tracing::error!("Failed to parse environment variable `{name}`.");
            None
        },
    }
}

/// Reads an environment variable containing the path to a Docker secret file.
#[must_use = "Secrets should be handled explicitly; ignoring the result may lead to misconfiguration."]
pub fn get_secret<T: std::str::FromStr>(
    name: &str,
) -> Option<T> {
    let path = get_env_internal(name)?;
    match read_secret_file(&path) {
        Ok(value) => {
            match value.parse() {
                Ok(parsed_value) => Some(parsed_value),
                Err(_) => {
                    tracing::error!("Failed to parse Docker secret `{name}`.");
                    None
                },
            }
        },
        Err(error) => {
            tracing::error!("Failed to read Docker secret: {error}");
            None
        },
    }
}

/// Reads an environment variable and returns its value if it exists; otherwise,
/// the `default` value is returned.
/// 
/// For more information, see [`get_env`].
#[must_use = "Ignoring the result may hide misconfigured or missing environment variables."]
pub fn get_env_or<T: std::str::FromStr>(
    name: &str,
    default: T,
    has_secret: bool,
) -> T {
    get_env(name, has_secret).unwrap_or(default)
}

/// Reads an environment variable and returns its value if it exists; otherwise,
/// the default value is returned.
/// 
/// For more information, see [`get_env`].
#[must_use = "Ignoring the result may hide misconfigured or missing environment variables."]
pub fn get_env_or_default<T: std::str::FromStr + Default>(
    name: &str,
    has_secret: bool,
) -> T {
    get_env(name, has_secret).unwrap_or_default()
}

/// Reads an environment variable and returns its value if it exists; otherwise,
/// this function will panic.
/// 
/// For more information, see [`get_env`].
/// 
/// # Panics
/// This function will panic if the environment variable does not exist.
#[must_use = "This function panics if the variable is missing; ignoring the result defeats its purpose."]
pub fn get_env_or_panic<T: std::str::FromStr>(
    name: &str,
    has_secret: bool,
) -> T {
    get_env(name, has_secret).unwrap_or_else(|| {
        panic!("Environment variable `{name}` is required but not set.");
    })
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        io::Write,
    };
    use serial_test::serial;
    use tempfile::NamedTempFile;
    use super::*;

    #[test]
    #[serial]
    fn test_get_env_internal() {
        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            env::set_var("TEST_ENV", "test_value");
        }
        assert_eq!(get_env_internal("TEST_ENV"), Some("test_value".to_string()));

        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            env::remove_var("TEST_ENV");
        }
        assert_eq!(get_env_internal("TEST_ENV"), None);
    }

    #[cfg(target_family = "unix")]
    #[test]
    #[serial]
    fn test_get_env_internal_invalid_unicode_linux() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        // Create a non-UTF8 environment variable value
        let invalid_utf8 = OsString::from_vec(vec![0xff, 0xfe, 0xfd]);
        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            std::env::set_var("INVALID_UNICODE_ENV", &invalid_utf8);
        }

        // Should log an error and return None
        assert_eq!(get_env_internal("INVALID_UNICODE_ENV"), None);

        // Clean up:
        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            std::env::remove_var("INVALID_UNICODE_ENV");
        }
    }

    #[test]
    #[serial]
    fn test_read_secret_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "secret_value").unwrap();

        let path = temp_file.path().to_str().unwrap();
        let result = read_secret_file(path).unwrap();

        assert_eq!(result, "secret_value");

        // Test with a non-existent file
        let invalid_path = "/invalid/path/to/secret";
        assert!(read_secret_file(invalid_path).is_err());
    }

    #[test]
    #[serial]
    fn test_get_env() {
        // Create Docker secret:
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "docker_secret_value").unwrap();

        // Set environment variables:
        let path = temp_file.path().to_str().unwrap();
        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            env::set_var("TEST_ENV_FILE", path); // Secret path
            env::set_var("TEST_ENV", "env_value"); // Non-secret value
        }

        // When has_secret is true, Docker secret is prioritised:
        assert_eq!(get_env("TEST_ENV", true), Some("docker_secret_value".to_string()));

        // When has_secret is false, environment variable is used:
        assert_eq!(get_env("TEST_ENV", false), Some("env_value".to_string()));

        // Clean up:
        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            env::remove_var("TEST_ENV_FILE");
            env::remove_var("TEST_ENV");
        }
    }

    #[test]
    #[serial]
    fn test_get_env_or_panic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "required_secret_value").unwrap();

        let path = temp_file.path().to_str().unwrap();
        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            env::set_var("REQUIRED_ENV_FILE", path);
        }

        // When has_secret is true, Docker secret is prioritised:
        assert_eq!(get_env_or_panic::<String>("REQUIRED_ENV", true), "required_secret_value".to_string());

        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            env::set_var("REQUIRED_ENV", "required_env_value");
        }

        // When has_secret is false, environment variable is used:
        assert_eq!(get_env_or_panic::<String>("REQUIRED_ENV", false), "required_env_value".to_string());

        // Test for missing variable:
        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            env::remove_var("REQUIRED_ENV_FILE");
            env::remove_var("REQUIRED_ENV");
        }

        let result = std::panic::catch_unwind(|| {
            _ = get_env_or_panic::<String>("REQUIRED_ENV", true);
        });
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_get_env_or_variants() {
        // SAFETY: Unit tests are run on the main thread.
        unsafe {
            env::remove_var("OPTIONAL_ENV");
        }

        assert_eq!(get_env_or("OPTIONAL_ENV", 123u32, false), 123);
        assert_eq!(get_env_or_default::<u32>("OPTIONAL_ENV", false), 0);

        // SAFETY:
        unsafe {
            env::set_var("OPTIONAL_ENV", "456");
        }

        assert_eq!(get_env_or("OPTIONAL_ENV", 123u32, false), 456);
        assert_eq!(get_env_or_default::<u32>("OPTIONAL_ENV", false), 456);
    }
}