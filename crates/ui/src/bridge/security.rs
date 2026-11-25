use axum::http::{HeaderMap, header};

pub fn generate_token() -> anyhow::Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow::anyhow!("generate bridge token: {error}"))?;
    Ok(hex::encode(bytes))
}

#[derive(Debug, Clone)]
pub struct Origins {
    app: String,
    dev: Option<String>,
}

impl Origins {
    pub fn new(port: u16) -> Self {
        let app = format!("http://127.0.0.1:{port}");
        let dev = std::env::var("TICONNECT_DEV_URL")
            .ok()
            .map(|url| url.trim_end_matches('/').to_owned())
            .filter(|url| url == "http://127.0.0.1:5173");
        Self { app, dev }
    }

    pub fn app(&self) -> &str {
        &self.app
    }

    pub fn dev(&self) -> Option<&str> {
        self.dev.as_deref()
    }

    pub fn dev_mode(&self) -> bool {
        self.dev.is_some()
    }

    pub fn allows(&self, origin: Option<&str>) -> bool {
        match origin {
            Some(origin) => origin == self.app || self.dev.as_deref() == Some(origin),
            None => self.dev_mode(),
        }
    }

    pub fn allows_navigation(&self, uri: &str) -> bool {
        if uri == "about:blank" {
            return true;
        }
        uri.starts_with(&format!("{}/", self.app))
            || uri == self.app
            || self
                .dev
                .as_deref()
                .is_some_and(|dev| uri.starts_with(&format!("{dev}/")) || uri == dev)
    }
}

pub fn host_is_loopback(headers: &HeaderMap, port: u16) -> bool {
    headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| {
            host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
        })
}

pub fn origin(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
}

pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_128_bits_of_hex() {
        let token = generate_token().expect("token");
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|character| character.is_ascii_hexdigit()));
    }

    #[test]
    fn origin_requires_exact_loopback() {
        let origins = Origins::new(4321);
        assert!(origins.allows(Some("http://127.0.0.1:4321")));
        assert!(!origins.allows(Some("http://127.0.0.1:4322")));
        assert!(!origins.allows(Some("https://127.0.0.1:4321")));
        assert!(!origins.allows(None));
    }
}
