use std::env;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let Some(url) = env::args().nth(1).filter(|arg| !arg.is_empty()) else {
        eprintln!("usage: kim-server-migrate <postgres-url>");
        return ExitCode::from(2);
    };
    let pool = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
    {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!("kim-server-migrate connect: {err}");
            return ExitCode::from(1);
        }
    };
    if let Err(err) = kim_server_schema::MIGRATOR.run(&pool).await {
        eprintln!("kim-server-migrate: {err}");
        return ExitCode::from(1);
    }
    pool.close().await;
    ExitCode::SUCCESS
}
