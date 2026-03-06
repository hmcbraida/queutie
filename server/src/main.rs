use server::Server;

fn main() -> std::io::Result<()> {
    let server = Server::new("127.0.0.1:3001")?;
    server.run();

    Ok(())
}
