#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let bind_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7878".into())
        .parse()?;
    unio_daemon::serve(bind_addr).await
}
