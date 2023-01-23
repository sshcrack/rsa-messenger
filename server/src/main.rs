
use std::net::{IpAddr, Ipv4Addr};

use clap::Parser;
use routes::router::serve_routes;
use crate::utils::types::*;

mod utils;
mod routes;
mod file;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    // Keep track of all connected users, key is usize, value
    // is a websocket sender.

    let users_list = UsersList::default();
    let users = Users::default();

    let args = Args::parse();

    let addr = args.bind.unwrap_or(IpAddr::V4(Ipv4Addr::new(127,0,0,1)));
    let port = args.port;

    serve_routes((addr, port), users, users_list).await;
}
