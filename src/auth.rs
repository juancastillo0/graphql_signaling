use std::borrow::Borrow;

use jsonwebtoken::{self as jwt, DecodingKey, EncodingKey};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

const SECRET: &'static str = "secret";

pub static AUTH: Lazy<AuthManager> = Lazy::new(|| AuthManager::new(jwt::Validation::default()));
pub struct AuthManager {
    jwt_encoding_key: EncodingKey,
    jwt_decoding_key: DecodingKey<'static>,
    jwt_validation: jwt::Validation,
    jwt_header: jwt::Header,
}

impl AuthManager {
    pub fn new(jwt_validation: jwt::Validation) -> Self {
        // let blacklist_db = db.open_tree(<BlackListedKey as KeyOf>::PREFIX).unwrap();
        // let mut interval = tokio::time::interval(time::Duration::new(60 * 5, 0)); // 5 min
        // let interval_db = blacklist_db.clone();
        // let handle = tokio::spawn(async move {
        //     loop {
        //         interval.tick().await;
        //         clear_blacklist(&interval_db).await;
        //     }
        // });
        let algorithm = jwt_validation.algorithms[0].clone();
        AuthManager {
            // blacklist_db: blacklist_db.clone(),
            jwt_encoding_key: EncodingKey::from_secret(SECRET.as_ref()),
            jwt_decoding_key: DecodingKey::from_secret(SECRET.as_ref()),
            jwt_validation,
            jwt_header: jwt::Header::new(algorithm),
            // _blacklist_interval_handle: handle,
        }
    }

    pub fn make_cookie(&self, user_id: String) -> (String, String) {
        use std::time;
        let max_age_secs = 2 * 24 * 60 * 60; // 2 days
        let exp = time::SystemTime::now()
            .checked_add(time::Duration::from_secs(max_age_secs))
            .unwrap()
            .duration_since(time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        let claims = Claims { exp, user_id };
        let token = jwt::encode(&self.jwt_header, &claims, &self.jwt_encoding_key).unwrap();
        let cookie = format!(
            "{}={}; Max-Age={}; SameSite=Lax; HttpOnly",
            SessionId::cookie_name(),
            &token,
            max_age_secs * 1000
        );

        (token, cookie)
    }

    pub fn session_id_from_cookie(&self, cookie: &str) -> Option<SessionId> {
        let data = jwt::decode::<Claims>(cookie, &self.jwt_decoding_key, &self.jwt_validation);
        data.ok().map(|claim| SessionId(claim.claims.user_id))
    }
}
#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    exp: u64, // seconds
    user_id: String,
}
pub struct SessionId(pub String);

impl SessionId {
    pub fn cookie_name() -> &'static str {
        "signaling-session-id"
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl ToString for SessionId {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}
