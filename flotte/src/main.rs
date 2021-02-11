mod raft;
mod structs;

pub mod raftproto {
    tonic::include_proto!("raft");
}

#[tokio::main]
async fn main() {
    let mut peers = Vec::new();
    peers.push("http://[::1]:5000");
    raft::Raft::spawn(0, peers);
}
