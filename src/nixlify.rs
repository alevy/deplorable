use bytes::Bytes;
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};

use crate::config;
use crate::server;

#[derive(Clone)]
pub struct Nixlify {
    conditions: BTreeMap<String, Arc<(Mutex<bool>, Condvar, Option<String>)>>,
}

impl Nixlify {
    pub fn new(config: config::Config) -> Nixlify {
        let conditions = {
            let mut map = BTreeMap::new();
            for (slug, repo) in config.repos.iter() {
                let pair = Arc::new((Mutex::new(false), Condvar::new(), repo.secret.clone()));
                let tpair = pair.clone();
                let repo = repo.clone();
                map.insert(slug.clone(), pair);
                std::thread::spawn(move || {
                    let _ = repo.build();
                    let (lock, cvar, _) = &*tpair;
                    loop {
                        {
                            let mut started = lock.lock().unwrap();
                            while !*started {
                                started = cvar.wait(started).unwrap();
                            }
                            *started = false;
                        }
                        let _ = repo.build();
                    }
                });
            }
            map
        };
        Nixlify { conditions }
    }
}

impl server::Handler for Nixlify {
    fn handle_request(&mut self, request: &http::Request<Bytes>) -> http::Response<Bytes> {
        let path = request.uri().path();
        if let Some(pair) = self.conditions.get(&path[1..].to_string()) {
            let (lock, cvar, secret) = &**pair;
            if verify_github_request(
                secret,
                &request.body(),
                request
                    .headers()
                    .get("x-hub-signature")
                    .map(|v| v.as_bytes()),
            ) {
                let mut started = lock.lock().unwrap();
                *started = true;
                cvar.notify_one();
                http::Response::builder().body(request.body().clone()).unwrap()
            } else {
                http::Response::builder().status(http::status::StatusCode::UNAUTHORIZED).body(Bytes::new()).unwrap()
            }
        } else {
            http::Response::builder().status(http::status::StatusCode::NOT_FOUND).body(Bytes::new()).unwrap()
        }
    }
}

fn verify_github_request(secret: &Option<String>, payload: &Bytes, tag: Option<&[u8]>) -> bool {
    use ring::hmac;
    if let Some(secret) = secret {
        if let Some(tag) = tag {
            let tag = String::from_utf8_lossy(tag);
            let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret.as_bytes());
            tag.starts_with("sha1=")
                && hmac::verify(&key, payload.as_ref(), tag[5..].as_bytes()).is_ok()
        } else {
            false
        }
    } else {
        true
    }
}
