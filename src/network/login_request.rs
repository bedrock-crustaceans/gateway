use std::io::Read;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bedrock::auth::auth_identity::{AuthData, AuthDataClaims};
use bedrock::auth::auth_oidc::AuthOIDC;
use bedrock::protocol::ProtoCodecLE;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use p384::PublicKey;
use p384::pkcs8::DecodePublicKey;

#[derive(Clone, Debug)]
pub struct RequestData {
    online: bool,
    key: PublicKey,
    auth_data: AuthDataClaims,
    client_data: serde_json::Value,
}

fn decode_request<R: Read>(stream: &mut R, oidc: Option<&AuthOIDC>) -> Option<RequestData> {
    let auth_data_buf = {
        let len = <i32 as ProtoCodecLE>::deserialize(stream).ok()?;
        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).ok()?;
        buf
    };
    let auth_data = serde_json::from_slice::<AuthData>(&auth_data_buf).ok()?;

    let (online, claims) = auth_data.validate(oidc).ok()?;

    let der = BASE64_STANDARD.decode(&claims.cpk).ok()?;
    let key = PublicKey::from_public_key_der(&der).ok()?;

    let client_data_buf = {
        let len = <i32 as ProtoCodecLE>::deserialize(stream).ok()?;
        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).ok()?;
        buf
    };

    // we use sec1 because DecodingKey uses `from_sec1_bytes` internally instead of `from_public_key_der`. misleading method name
    let dec_key = DecodingKey::from_ec_der(&key.to_sec1_bytes());

    let mut validator = Validation::new(Algorithm::ES384);
    validator.required_spec_claims.remove("exp");
    validator.validate_exp = false;

    let data = decode::<serde_json::Value>(&client_data_buf, &dec_key, &validator).ok()?;

    Some(RequestData {
        online,
        key,
        auth_data: claims,
        client_data: data.claims,
    })
}
