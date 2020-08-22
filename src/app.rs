use bytes::Bytes;
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};

use crate::config;
use crate::server;

struct Worker {
    repo: config::Repo,
    condition: Arc<(Mutex<bool>, Condvar)>
}

impl Worker {
    fn spawn(self) {
        std::thread::spawn(move || {
            let _ = self.repo.build();
            let (lock, cvar) = &*self.condition;
            loop {
                let mut started = lock.lock().unwrap();
                while !*started {
                    started = cvar.wait(started).unwrap();
                }
                *started = false;
                drop(started); // releases the mutex
                let _ = self.repo.build();
            }
        });
    }
}

#[derive(Clone)]
pub struct Deplorable {
    conditions: BTreeMap<String, (Arc<(Mutex<bool>, Condvar)>, Option<String>)>,
}

impl Deplorable {
    pub fn new(config: config::Config) -> Self {
        let conditions = {
            let mut map = BTreeMap::new();
            for (slug, repo) in config.repos.iter() {
                let condition = Arc::new((Mutex::new(false), Condvar::new()));
                let worker = Worker {
                    repo: repo.clone(),
                    condition: condition.clone(),
                };
                map.insert(slug.clone(), (condition, repo.secret.clone()));
                worker.spawn();
            }
            map
        };
        Deplorable { conditions }
    }
}

impl server::Handler for Deplorable {
    fn handle_request(&mut self, request: &http::Request<Bytes>) -> http::Response<Bytes> {
        let path = request.uri().path();
        if let Some((condition, secret)) = self.conditions.get(&path[1..].to_string()) {
            let (lock, cvar) = &**condition;
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
                http::Response::builder()
                    .body(request.body().clone())
                    .unwrap()
            } else {
                http::Response::builder()
                    .status(http::status::StatusCode::UNAUTHORIZED)
                    .body(Bytes::new())
                    .unwrap()
            }
        } else {
            http::Response::builder()
                .status(http::status::StatusCode::NOT_FOUND)
                .body(Bytes::new())
                .unwrap()
        }
    }
}

pub fn from_hex(hex_str: &str) -> Result<Vec<u8>, ()> {
    fn from_digit(digit: u8) -> Result<u8, ()> {
        match digit {
            b'0'..=b'9' => Ok(digit - b'0'),
            b'A'..=b'F' => Ok(10 + digit - b'A'),
            b'a'..=b'f' => Ok(10 + digit - b'a'),
            _ => Err(()),
        }
    }

    if hex_str.len() & 1 != 0 {
        return Err(());
    }

    let mut result = Vec::with_capacity(hex_str.len() / 2);
    for digits in hex_str.as_bytes().chunks(2) {
        let hi = from_digit(digits[0])?;
        let lo = from_digit(digits[1])?;
        result.push((hi << 4) | lo);
    }
    Ok(result)
}

fn verify_github_request(secret: &Option<String>, payload: &Bytes, tag: Option<&[u8]>) -> bool {
    use ring::hmac;
    if let Some(secret) = secret {
        if let Some(tag) = tag {
            let tag = String::from_utf8_lossy(tag);
            let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret.as_bytes());
            if let Ok(tagbytes) = from_hex(&tag[5..]) {
                tag.starts_with("sha1=")
                    && hmac::verify(&key, payload.as_ref(), tagbytes.as_slice()).is_ok()
            } else {
                false
            }
        } else {
            false
        }
    } else {
        true
    }
}
