use std::net::TcpListener;
use std::sync::{Arc, Condvar, Mutex};

mod nixhub;
mod http;

fn main() -> Result<(), std::io::Error> {
    use clap::{App, Arg};
    let arg_matches = App::new("Nixhub Builder")
        .arg(
            Arg::with_name("repo")
                .short("R")
                .long("repo")
                .value_name("REPOSITORY")
                .help("Repository name")
                .required(true)
                .display_order(1)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ref")
                .short("r")
                .long("ref")
                .value_name("REFERENCE")
                .help("Git branch, version or commit hash")
                .required(true)
                .display_order(2)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("out")
                .short("o")
                .long("out")
                .value_name("OUT_LINK")
                .default_value("result")
                .display_order(4)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("token")
                .short("t")
                .long("auth-token")
                .value_name("TOKEN")
                .help("GitHub OAuth token (only necessary for private repositories)")
                .display_order(3)
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

    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let repo = String::from(arg_matches.value_of("repo").expect("repo"));
    let nh: nixhub::Nixhub = nixhub::Nixhub {
        repo: repo.clone(),
        reference: arg_matches.value_of("ref").expect("ref").to_string(),
        out: arg_matches.value_of("out").expect("out").to_string(),
        token: arg_matches
            .value_of("token")
            .map(|t| t.to_string())
            .or_else(|| std::env::var("GITHUB_TOKEN").ok()),
    };

    let pair2 = pair.clone();
    std::thread::spawn(move || {
        let (lock, cvar) = &*pair2;
        loop {
            {
                let mut started = lock.lock().unwrap();
                while !*started {
                    started = cvar.wait(started).unwrap();
                }
                *started = false;
            }
            let _ = nh.build();
        }
    });

    let listen = arg_matches.value_of("listen").expect("listen").clone();
    let listener = TcpListener::bind(listen)?;
    for stream in listener.incoming() {
        let stream = stream?;
        let pair = pair.clone();
        let repo = repo.clone();
        std::thread::spawn(move || {
            let (lock, cvar) = &*pair;
            let mut client = http::Client::new(stream); 
            if let Ok((request, body)) = client.read() {
                if let Some(path) = request.path {
                    if &path[1..] == repo {
                        let mut started = lock.lock().unwrap();
                        *started = true;
                        cvar.notify_one();
                    }
                }
                let _ = client.respond_ok(body);
            }
        });
    }

    Ok(())
}
