use server::Server;

fn main() -> std::io::Result<()> {
    let server = Server::new("127.0.0.1:3001", 8, 10_000)?;
    server.run();

    Ok(())
}
