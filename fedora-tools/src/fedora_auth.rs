use reqwest::Url;
use anyhow::{Result};

pub struct FedoraAuth {
    pub username: String,
    pub password: String,
    pub root_url: Url,
}

impl FedoraAuth {
    /* Returns uri::Error if root_uri fails to parse */
    pub fn new(username: &str, password: &str, root_uri: &str) -> Result<Self> {
        let url = Url::parse(root_uri)?;
        Ok(Self {
            username: username.to_owned(),
            password: password.to_owned(),
            root_url: url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fails_with_invalid_uri() {
        let f_auth = FedoraAuth::new("username", "password", "://localhost:8080/rest/development");
        assert!(f_auth.is_err());
    }
}
