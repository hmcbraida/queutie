use server::Server;

fn main() {
    let server = Server::new("127.0.0.1:3001").unwrap();
    server.run();
}
