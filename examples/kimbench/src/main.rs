use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use kimbench::{run_group, run_login, run_user, BenchOpts};
use pkt_client::resolve_jwt_secret;

#[derive(Parser, Debug)]
#[command(name = "kimbench", about = "KIM login / 1:1 / group bench")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Login(Flags),
    User(Flags),
    Group(GroupFlags),
}

#[derive(clap::Args, Debug)]
struct Flags {
    #[arg(short = 'a', long = "address", default_value = "ws://127.0.0.1:8001/")]
    address: String,
    #[arg(short = 's', long = "app-secret")]
    app_secret: Option<String>,
    #[arg(long = "app", default_value = "kim")]
    app: String,
    #[arg(short = 'c', long = "count", default_value_t = 100)]
    count: u64,
    #[arg(short = 't', long = "threads", default_value_t = 10)]
    threads: usize,
    #[arg(long = "timeout", default_value = "10s")]
    timeout: String,
    #[arg(short = 'k', long = "keep", default_value = "0s")]
    keep: String,
}

#[derive(clap::Args, Debug)]
struct GroupFlags {
    #[command(flatten)]
    flags: Flags,
    #[arg(short = 'm', long = "members", default_value_t = 20)]
    members: usize,
    #[arg(short = 'p', long = "online", default_value_t = 0.5)]
    online: f64,
}

fn parse_dur(s: &str) -> Duration {
    humantime::parse_duration(s).unwrap_or(Duration::from_secs(10))
}

fn opts_from(f: &Flags, members: usize, online: f64) -> BenchOpts {
    BenchOpts {
        address: f.address.clone(),
        secret: f
            .app_secret
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(resolve_jwt_secret),
        app: f.app.clone(),
        count: f.count,
        threads: f.threads.max(1),
        timeout: parse_dur(&f.timeout),
        keep: parse_dur(&f.keep),
        members,
        online,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let cli = Cli::parse();
    let wall_start = Instant::now();
    let stats = match cli.cmd {
        Command::Login(f) => run_login(opts_from(&f, 0, 0.0)).await?,
        Command::User(f) => run_user(opts_from(&f, 0, 0.0)).await?,
        Command::Group(g) => run_group(opts_from(&g.flags, g.members, g.online)).await?,
    };
    print!("{}", stats.render(wall_start.elapsed()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_login() {
        let cli = Cli::try_parse_from(["kimbench", "login", "-c", "4", "-t", "2"]).unwrap();
        match cli.cmd {
            Command::Login(f) => {
                assert_eq!(f.count, 4);
                assert_eq!(f.threads, 2);
            }
            _ => panic!("expected login"),
        }
    }
}
