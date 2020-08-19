use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::net::SocketAddr;

#[derive(Clone)]
struct Nixhub {
    owner: String,
    repo: String,
    reference: String,
    out: String,
    token: Option<String>,
}

impl Nixhub {
    async fn request_tarball_location(
        &self,
    ) -> Result<reqwest::header::HeaderValue, reqwest::Error> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/tarball/{}",
            self.owner, self.repo, self.reference
        );
        let client = reqwest::Client::builder()
            .user_agent("Nix builder")
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut request = client.get(&url);
        if let Some(token) = self.token.as_ref() {
            request = request.header("Authorization", format!("token {}", token));
        }
        let resp = request.send().await?;
        Ok(resp.headers()["location"].clone())
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    use clap::{App, Arg};
    let arg_matches = App::new("Nixhub Builder")
        .arg(
            Arg::with_name("owner")
                .short("O")
                .long("owner")
                .value_name("OWNER")
                .help("Repository owner")
                .required(true)
                .display_order(0)
                .takes_value(true),
        )
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

    let nixhub: &'static Nixhub = Box::leak(Box::new(Nixhub {
        owner: String::from(arg_matches.value_of("owner").expect("owner")),
        repo: String::from(arg_matches.value_of("repo").expect("repo")),
        reference: arg_matches.value_of("ref").expect("ref").to_string(),
        out: arg_matches.value_of("out").expect("out").to_string(),
        token: arg_matches
            .value_of("token")
            .map(|t| t.to_string())
            .or_else(|| std::env::var("GITHUB_TOKEN").ok()),
    }));
    if let Ok(location) = nixhub.request_tarball_location().await {
        std::process::Command::new("nix-build")
            .arg("--out-link")
            .arg(&nixhub.out)
            .arg(location.to_str().unwrap())
            .spawn()?
            .wait()?;
    }
    let listen = arg_matches.value_of("listen").expect("listen").clone();

    let addr: SocketAddr = listen.parse().expect("Couldn't parse listen address");

    let svc = make_service_fn(|_| async move {
        Ok::<_, std::io::Error>(service_fn(move |_: Request<Body>| async move {
            if let Ok(location) = nixhub.request_tarball_location().await {
                std::process::Command::new("nix-build")
                    .arg("--out-link")
                    .arg(&nixhub.out)
                    .arg(location.to_str().unwrap())
                    .spawn()?
                    .wait()?;
            }
            Ok::<_, std::io::Error>(Response::new(Body::empty()))
        }))
    });

    let server = Server::bind(&addr).serve(svc);
    async fn shutdown_signal() {
        // Wait for the CTRL+C signal
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C signal handler");
    }
    let graceful = server.with_graceful_shutdown(shutdown_signal());
    println!("Listening at {}", addr);
    // Run this server for... forever!
    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }
    Ok(())
}
