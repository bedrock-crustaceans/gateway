use bedrock::auth::connection_request::ConnectionRequest;
use bedrock::network::encryption::Encryption;
use bedrock::network::login::handshake::client_finish_encryption;
use p384::elliptic_curve::Generate;
use p384::SecretKey;

pub struct ProxyKeys {
    secret: SecretKey,
}

impl ProxyKeys {
    pub fn generate() -> Self {
        Self {
            secret: SecretKey::generate_from_rng(&mut rand::rng()),
        }
    }

    pub fn forge_login(&self, request: &[u8]) -> Option<Vec<u8>> {
        ConnectionRequest::parse(request).ok()?.resigned(&self.secret).ok()?.to_bytes().ok()
    }

    pub fn server_encryption(&self, handshake_jwt: &str) -> Option<Encryption> {
        client_finish_encryption(handshake_jwt, &self.secret).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock::auth::chain::ChainRoot;
    use bedrock::auth::client_data::ClientData;
    use bedrock::network::login::handshake::server_begin_encryption;

    fn client_login() -> Vec<u8> {
        let client = SecretKey::generate_from_rng(&mut rand::rng());
        let client_data = ClientData::offline("1.21.0", "127.0.0.1:19132", "Steve");
        ConnectionRequest::self_signed(&client, "Steve", &client_data).unwrap().to_bytes().unwrap()
    }

    #[test]
    fn forged_login_verifies_under_the_proxy_key() {
        let proxy = ProxyKeys::generate();

        let forged = ConnectionRequest::parse(&proxy.forge_login(&client_login()).unwrap()).unwrap();
        let login = forged.verify(None, &ChainRoot::default()).unwrap();

        assert_eq!(login.authentication.identity().display_name, "Steve");
        assert_eq!(login.authentication.identity().public_key().unwrap(), proxy.secret.public_key());
    }

    #[test]
    fn garbage_login_is_not_forged() {
        assert!(ProxyKeys::generate().forge_login(&[1, 2, 3]).is_none());
    }

    #[test]
    fn server_handshake_derives_matching_encryption() {
        let proxy = ProxyKeys::generate();
        let server = SecretKey::generate_from_rng(&mut rand::rng());
        let (token, mut theirs) = server_begin_encryption(&proxy.secret.public_key(), &server, &[7; 16]).unwrap();

        let mut ours = proxy.server_encryption(&token).unwrap();

        let encrypted = ours.encrypt(b"hello".to_vec()).unwrap();
        assert_eq!(theirs.decrypt(encrypted).unwrap(), b"hello");
    }
}
