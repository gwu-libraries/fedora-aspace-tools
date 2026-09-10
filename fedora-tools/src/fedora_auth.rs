use uris::{Error, Uri};

pub struct FedoraAuth {
    pub username: String,
    pub password: String,
    pub root_uri: Uri,
}

impl FedoraAuth {
    /* Returns uri::Error if root_uri fails to parse */
    pub fn new(username: &str, password: &str, root_uri: &str) -> Result<Self, Error> {
        let uri = Uri::parse(root_uri)?;
        Ok(Self {
            username: username.to_owned(),
            password: password.to_owned(),
            root_uri: uri,
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
