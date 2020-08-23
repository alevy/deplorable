use std::net::TcpListener;

mod app;
mod config;
mod server;

use app::Deplorable;

fn main() -> Result<(), std::io::Error> {
    use clap::Arg;
    let arg_matches = clap::App::new("Nixhub Builder")
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
    let listen = arg_matches.value_of("listen").expect("listen").clone();

    let app = Deplorable::new(config);
    let server = server::Server::new(TcpListener::bind(listen)?, app);
    server.run()
}
