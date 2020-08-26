use bytes::Bytes;
use http::status::StatusCode;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};

use crate::config;
use crate::server;

struct Worker {
    repo: config::Repo,
    condition: Arc<(Mutex<bool>, Condvar)>,
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

#[derive(Deserialize)]
struct GitHubPushEvent {
    #[serde(rename(deserialize = "ref"))]
    reference: String,
}

#[derive(Clone)]
pub struct Deplorable {
    conditions: BTreeMap<String, (Arc<(Mutex<bool>, Condvar)>, config::Repo)>,
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
                map.insert(slug.clone(), (condition, repo.clone()));
                worker.spawn();
            }
            map
        };
        Deplorable { conditions }
    }

    fn handle_repo_request(
        &self,
        request: &http::Request<Bytes>,
        path: &str,
    ) -> DeplorableResult<()> {
        let (condition, repo) = self
            .conditions
            .get(&path[1..].to_string())
            .ok_or(http::StatusCode::NOT_FOUND)?;
        let (lock, cvar) = &**condition;

        verify_github_request(
            &repo.secret,
            &request.body(),
            request
                .headers()
                .get("x-hub-signature")
                .map(|v| v.as_bytes()),
        )?;

        let event_type = request
            .headers()
            .get("x-github-event")
            .ok_or(StatusCode::BAD_REQUEST)?;
        match event_type.as_bytes() {
            b"push" => {
                let event_body: GitHubPushEvent =
                    serde_yaml::from_slice(request.body().as_ref()).or(Err(StatusCode::BAD_REQUEST))?;
                if event_body.reference != repo.reference {
                    Err(StatusCode::BAD_REQUEST)
                } else {
                    println!("Push request for {}", repo.repo);
                    let mut started = lock.lock().unwrap();
                    *started = true;
                    cvar.notify_one();
                    Ok(())
                }
            },
            _ => Ok(())
        }
    }
}

type DeplorableResult<T> = Result<T, StatusCode>;

impl server::Handler for Deplorable {
    fn handle_request(&mut self, request: &http::Request<Bytes>) -> http::Response<Bytes> {
        let path = request.uri().path();
        match self.handle_repo_request(request, path) {
            Ok(_) => http::Response::builder()
                .body(request.body().clone())
                .unwrap(),
            Err(status_code) => http::Response::builder()
                .status(status_code)
                .body(Bytes::new())
                .unwrap(),
        }
    }
}

fn from_hex(hex_str: &str) -> DeplorableResult<Vec<u8>> {
    fn from_digit(digit: u8) -> DeplorableResult<u8> {
        match digit {
            b'0'..=b'9' => Ok(digit - b'0'),
            b'A'..=b'F' => Ok(10 + digit - b'A'),
            b'a'..=b'f' => Ok(10 + digit - b'a'),
            _ => return Err(StatusCode::BAD_REQUEST),
        }
    }

    if hex_str.len() & 1 != 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut result = Vec::with_capacity(hex_str.len() / 2);
    for digits in hex_str.as_bytes().chunks(2) {
        let hi = from_digit(digits[0])?;
        let lo = from_digit(digits[1])?;
        result.push((hi << 4) | lo);
    }
    Ok(result)
}

fn verify_github_request(
    secret: &Option<String>,
    payload: &Bytes,
    tag: Option<&[u8]>,
) -> DeplorableResult<()> {
    use ring::hmac;
    if let Some(secret) = secret {
        if let Some(tag) = tag {
            let tag = String::from_utf8_lossy(tag);
            let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret.as_bytes());
            let tagbytes = from_hex(&tag[5..])?;
            if tag.starts_with("sha1=") {
                hmac::verify(&key, payload.as_ref(), tagbytes.as_slice())
                    .or(Err(StatusCode::UNAUTHORIZED))
            } else {
                Err(StatusCode::BAD_REQUEST)
            }
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    } else {
        Ok(())
    }
}
