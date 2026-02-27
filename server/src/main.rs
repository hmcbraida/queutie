use std::net::TcpListener;

use queutie_common::network;

struct Configuration {
    hostname: String,
    port: u16,
}

fn parse_args<T: Iterator<Item = String>>(mut args: T) -> Configuration {
    args.next().unwrap();
    let hostname = args.next().unwrap();
    let port: u16 = args.next().unwrap().parse().unwrap();

    Configuration { hostname, port }
}

fn main() {
    let configuration = parse_args(std::env::args());

    let listener =
        TcpListener::bind((configuration.hostname.as_str(), configuration.port)).unwrap();

    for mut stream in listener.incoming().map(|x| x.unwrap()) {
        let packet = network::read_packet(&mut stream);
        println!("{:?}", packet);
        println!("{}", str::from_utf8(&packet.body).unwrap())
    }
}
