use std::{env, net::SocketAddr};

use process_dashboard::{app_router, build_pool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://process-dashboard.db".to_string());
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".to_string());

    let pool = build_pool(&database_url).await?;
    let app = app_router(pool);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    let socket_addr: SocketAddr = listener.local_addr()?;
    println!("process-dashboard listening on http://{socket_addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
