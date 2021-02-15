mod raft;
mod structs;
mod server;
mod vote;
mod handlers;
mod replication;

use server::FlotteService;
use tonic::transport::Server;
use tracing_subscriber::prelude::*;

pub mod raftproto {
    tonic::include_proto!("raft");
}

#[tokio::main]
async fn main() {
    let fmt_layer = tracing_subscriber::fmt::Layer::default()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .with_ansi(false);
    let subscriber = tracing_subscriber::Registry::default()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber).expect("error setting global tracing subscriber");

    tracing::info!("--- initializing cluster");

    let id = std::env::args().nth(1).expect("no id given");
    let addr = std::env::args().nth(2).expect("no addr given");

    let server = FlotteService::new(id.parse::<u64>().unwrap()).await;
    Server::builder()
    .add_service(raftproto::raft_server::RaftServer::new(server))
    .serve(addr.parse().unwrap())
    .await.unwrap();
}