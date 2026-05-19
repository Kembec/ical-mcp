use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::fmt;

#[derive(Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

impl Credentials {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Result<Self> {
        let username = username.into();
        let password = password.into();
        if username.trim().is_empty() {
            return Err(anyhow!("ICLOUD_USERNAME is empty"));
        }
        if password.trim().is_empty() {
            return Err(anyhow!("ICLOUD_PASSWORD is empty"));
        }
        Ok(Self { username, password })
    }
}

pub fn load() -> Result<Credentials> {
    let username = std::env::var("ICLOUD_USERNAME")
        .map_err(|_| anyhow!("ICLOUD_USERNAME environment variable is not set"))?;
    let password = std::env::var("ICLOUD_PASSWORD").map_err(|_| {
        anyhow!(
            "ICLOUD_PASSWORD environment variable is not set. \
             Generate an app-specific password at https://appleid.apple.com/account/manage"
        )
    })?;
    Credentials::new(username, password)
}

pub fn basic_auth_header(creds: &Credentials) -> String {
    let raw = format!("{}:{}", creds.username, creds.password);
    format!("Basic {}", B64.encode(raw.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_username() {
        assert!(Credentials::new("", "x").is_err());
    }

    #[test]
    fn rejects_empty_password() {
        assert!(Credentials::new("a@b.c", "").is_err());
    }

    #[test]
    fn basic_auth_encodes_correctly() {
        let creds = Credentials::new("alice", "secret").unwrap();
        // base64("alice:secret") = "YWxpY2U6c2VjcmV0"
        assert_eq!(basic_auth_header(&creds), "Basic YWxpY2U6c2VjcmV0");
    }
}
