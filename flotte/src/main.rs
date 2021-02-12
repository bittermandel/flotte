mod raft;
mod structs;
mod server;

use server::FlotteService;
use tonic::transport::Server;

pub mod raftproto {
    tonic::include_proto!("raft");
}

#[tokio::main]
async fn main() {
    let server = FlotteService::new(0).await;
    Server::builder()
    .add_service(raftproto::request_vote_server::RequestVoteServer::new(server))
    .serve("[::1]:8000".parse().unwrap())
    .await.unwrap();
}
