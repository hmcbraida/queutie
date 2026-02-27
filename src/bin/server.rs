use std::{env, net::TcpListener};

struct Configuration {
    hostname: String,
    port: u16,
}

fn parse_args<T: Iterator<Item = String>>(mut args: T) -> Configuration {
    let hostname = args.next().unwrap();
    let port: u16 = args.next().unwrap().parse().unwrap();

    Configuration { hostname, port }
}

fn main() {
    let configuration = parse_args(std::env::args());

    let listener = TcpListener::bind((configuration.hostname.as_str(), configuration.port));
}
