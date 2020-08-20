use bytes::Bytes;
use std::collections::BTreeMap;
use std::net::TcpListener;
use std::sync::{Arc, Condvar, Mutex};

mod config;
mod server;

fn main() -> Result<(), std::io::Error> {
    use clap::{App, Arg};
    let arg_matches = App::new("Nixhub Builder")
        .arg(
            Arg::with_name("config file")
                .short("c")
                .long("config")
                .value_name("PATH_TO_CONFIG_FILE")
                .help("Path to YAML formatted configuration file")
                .default_value("config.yaml")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("listen")
                .short("l")
                .long("listen")
                .value_name("ADDR:PORT")
                .help("Address and port to listen on")
                .default_value("0.0.0.0:1337")
                .takes_value(true),
        )
        .get_matches();

    let config: config::Config = {
        let config_file = std::fs::File::open(
            arg_matches
                .value_of("config file")
                .expect("config file")
                .clone(),
        )?;
        serde_yaml::from_reader(config_file)
            .map_err(|e| eprintln!("{:?}", e))
            .unwrap()
    };
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

    let listen = arg_matches.value_of("listen").expect("listen").clone();
    let listener = TcpListener::bind(listen)?;
    for stream in listener.incoming() {
        let stream = stream?;
        let conditions = conditions.clone();
        std::thread::spawn(move || {
            let mut client = server::Client::new(stream);
            let empty_body = Bytes::new();
            loop {
                match client.read() {
                    Ok(request) => {
                        let path = request.uri().path();
                        let response = if let Some(pair) = conditions.get(&path[1..].to_string()) {
                            let (lock, cvar, secret) = &**pair;
                            if verify_request(
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
                                http::Response::builder().body(request.body())
                            } else {
                                http::Response::builder().status(http::status::StatusCode::UNAUTHORIZED).body(&empty_body)
                            }
                        } else {
                            http::Response::builder().status(http::status::StatusCode::NOT_FOUND).body(&empty_body)
                        };
                        let _ = client.write_response(&response.unwrap());
                    },
                    Err(r) => {
                        if r.kind() != std::io::ErrorKind::UnexpectedEof {
                            eprintln!("{}", r);
                        }
                        break;
                    },
                }
            }
        });
    }

    Ok(())
}

fn verify_request(secret: &Option<String>, payload: &Bytes, tag: Option<&[u8]>) -> bool {
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
