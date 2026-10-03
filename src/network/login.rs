use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use bedrock::network::encryption::Encryption;
use p384::ecdsa::signature::Signer;
use p384::ecdsa::{Signature, SigningKey};
use p384::elliptic_curve::Generate;
use p384::pkcs8::{DecodePublicKey, EncodePublicKey};
use p384::{PublicKey, SecretKey};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

const AUTH_TYPE_SELF_SIGNED: u8 = 2;

pub struct ProxyKeys {
    secret: SecretKey,
    public_b64: String,
}

impl ProxyKeys {
    pub fn generate() -> Self {
        let secret = SecretKey::generate_from_rng(&mut rand::rng());
        let der = secret.public_key().to_public_key_der().expect("valid P-384 public key");
        Self {
            public_b64: STANDARD.encode(der.as_bytes()),
            secret,
        }
    }

    fn sign_jwt(&self, payload_b64: &str) -> String {
        let header = json!({ "alg": "ES384", "x5u": self.public_b64 });
        let message = format!("{}.{}", URL_SAFE_NO_PAD.encode(header.to_string()), payload_b64);
        let signature: Signature = SigningKey::from(&self.secret).sign(message.as_bytes());
        format!("{message}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
    }

    pub fn forge_login(&self, request: &[u8]) -> Option<Vec<u8>> {
        let (auth, rest) = read_lpstr(request)?;
        let (client_data, _) = read_lpstr(rest)?;

        let auth: Value = serde_json::from_slice(auth).ok()?;
        let extra_data = extract_identity(&auth)?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        let claims = json!({
            "certificateAuthority": true,
            "identityPublicKey": self.public_b64,
            "extraData": extra_data,
            "iat": now,
            "nbf": now - 60,
            "exp": now + 24 * 60 * 60,
        });
        let chain = self.sign_jwt(&URL_SAFE_NO_PAD.encode(claims.to_string()));

        let mut token_claims = original_token_claims(&auth).unwrap_or_else(|| {
            json!({
                "xid": extra_data["XUID"],
                "xname": extra_data["displayName"],
                "mid": "",
            })
        });
        token_claims["cpk"] = json!(self.public_b64);
        token_claims["iat"] = json!(now);
        token_claims["nbf"] = json!(now - 60);
        token_claims["exp"] = json!(now + 24 * 60 * 60);
        let token = self.sign_jwt(&URL_SAFE_NO_PAD.encode(token_claims.to_string()));

        let auth = json!({
            "AuthenticationType": AUTH_TYPE_SELF_SIGNED,
            "Certificate": json!({ "chain": [chain] }).to_string(),
            "Token": token,
        })
        .to_string();

        let client_data_payload = std::str::from_utf8(client_data).ok()?.split('.').nth(1)?;
        let client_data = self.sign_jwt(client_data_payload);

        let mut out = Vec::new();
        write_lpstr(&mut out, auth.as_bytes());
        write_lpstr(&mut out, client_data.as_bytes());
        Some(out)
    }

    pub fn server_encryption(&self, handshake_jwt: &str) -> Option<Encryption> {
        let mut parts = handshake_jwt.split('.');
        let header = decode_jwt_part(parts.next()?)?;
        let payload = decode_jwt_part(parts.next()?)?;

        let server_key = STANDARD.decode(header["x5u"].as_str()?).ok()?;
        let server_key = PublicKey::from_public_key_der(&server_key).ok()?;

        let salt = payload["salt"].as_str()?.trim_end_matches('=');
        let salt: [u8; 16] = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(salt)
            .ok()?
            .try_into()
            .ok()?;

        Some(Encryption::new(&self.secret, &server_key, &salt))
    }
}

fn original_token_claims(auth: &Value) -> Option<Value> {
    let token = auth["Token"].as_str().filter(|t| !t.is_empty())?;
    decode_jwt_part(token.split('.').nth(1)?)
}

fn extract_identity(auth: &Value) -> Option<Value> {
    if let Some(claims) = original_token_claims(auth) {
        let xuid = claims["xid"].as_str().unwrap_or_default();
        let name = claims["xname"].as_str()?;
        return Some(json!({
            "XUID": xuid,
            "displayName": name,
            "identity": offline_uuid(name),
        }));
    }

    let certificate: Value = serde_json::from_str(auth["Certificate"].as_str()?).ok()?;
    certificate["chain"]
        .as_array()?
        .iter()
        .filter_map(|jwt| decode_jwt_part(jwt.as_str()?.split('.').nth(1)?))
        .find_map(|claims| claims.get("extraData").cloned())
}

fn offline_uuid(name: &str) -> String {
    uuid::Uuid::new_v3(&uuid::Uuid::NAMESPACE_OID, name.as_bytes()).to_string()
}

fn decode_jwt_part(part: &str) -> Option<Value> {
    let bytes = URL_SAFE_NO_PAD.decode(part.trim_end_matches('=')).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn read_lpstr(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    let len = u32::from_le_bytes(buf.get(..4)?.try_into().ok()?) as usize;
    let rest = &buf[4..];
    Some((rest.get(..len)?, &rest[len..]))
}

fn write_lpstr(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use p384::ecdsa::signature::Verifier;
    use p384::ecdsa::VerifyingKey;

    fn verify(jwt: &str) -> Value {
        let parts: Vec<&str> = jwt.split('.').collect();
        let header = decode_jwt_part(parts[0]).unwrap();
        let der = STANDARD.decode(header["x5u"].as_str().unwrap()).unwrap();
        let key = VerifyingKey::from(PublicKey::from_public_key_der(&der).unwrap());
        let sig = Signature::from_slice(&URL_SAFE_NO_PAD.decode(parts[2]).unwrap()).unwrap();
        key.verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &sig).unwrap();
        decode_jwt_part(parts[1]).unwrap()
    }

    #[test]
    fn forged_login_is_self_signed() {
        let client = ProxyKeys::generate();
        let token_claims = json!({ "xid": "123", "xname": "Steve", "mid": "m", "cpk": client.public_b64 });
        let token = client.sign_jwt(&URL_SAFE_NO_PAD.encode(token_claims.to_string()));
        let client_data = client.sign_jwt(&URL_SAFE_NO_PAD.encode(r#"{"SkinId":"x"}"#));
        let auth = json!({ "AuthenticationType": 0, "Certificate": "", "Token": token }).to_string();

        let mut request = Vec::new();
        write_lpstr(&mut request, auth.as_bytes());
        write_lpstr(&mut request, client_data.as_bytes());

        let proxy = ProxyKeys::generate();
        let forged = proxy.forge_login(&request).unwrap();
        let (auth, rest) = read_lpstr(&forged).unwrap();
        let (client_data, _) = read_lpstr(rest).unwrap();

        let auth: Value = serde_json::from_slice(auth).unwrap();
        let certificate: Value = serde_json::from_str(auth["Certificate"].as_str().unwrap()).unwrap();
        let identity = verify(certificate["chain"][0].as_str().unwrap());
        assert_eq!(identity["identityPublicKey"], proxy.public_b64);
        assert_eq!(identity["extraData"]["displayName"], "Steve");
        assert_eq!(verify(std::str::from_utf8(client_data).unwrap())["SkinId"], "x");
    }

    #[test]
    fn forged_login_passes_bedrock_rs_validation() {
        let client = ProxyKeys::generate();
        let token_claims = json!({ "xid": "123", "xname": "Steve", "mid": "m", "cpk": client.public_b64 });
        let token = client.sign_jwt(&URL_SAFE_NO_PAD.encode(token_claims.to_string()));
        let auth = json!({ "AuthenticationType": 0, "Certificate": "", "Token": token }).to_string();

        let mut request = Vec::new();
        write_lpstr(&mut request, auth.as_bytes());
        write_lpstr(&mut request, client.sign_jwt("e30").as_bytes());

        let proxy = ProxyKeys::generate();
        let forged = proxy.forge_login(&request).unwrap();
        let (auth, _) = read_lpstr(&forged).unwrap();

        let auth: bedrock::auth::auth_identity::AuthData = serde_json::from_slice(auth).unwrap();
        let (online, claims) = auth.validate(None).unwrap();
        assert!(!online);
        assert_eq!(claims.cpk, proxy.public_b64);
        assert_eq!((claims.xname.as_str(), claims.xid.as_str()), ("Steve", "123"));
    }

    #[test]
    fn server_handshake_derives_matching_encryption() {
        let proxy = ProxyKeys::generate();
        let server = ProxyKeys::generate();
        let salt = [7u8; 16];
        let handshake = server.sign_jwt(&URL_SAFE_NO_PAD.encode(json!({ "salt": STANDARD.encode(salt) }).to_string()));

        let mut ours = proxy.server_encryption(&handshake).unwrap();
        let der = STANDARD.decode(&proxy.public_b64).unwrap();
        let mut theirs = Encryption::new(&server.secret, &PublicKey::from_public_key_der(&der).unwrap(), &salt);

        let encrypted = ours.encrypt(b"hello".to_vec()).unwrap();
        assert_eq!(theirs.decrypt(encrypted).unwrap(), b"hello");
    }
}
