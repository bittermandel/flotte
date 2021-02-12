use anyhow::Result;

use tokio::{sync::{mpsc::{self, UnboundedSender}, oneshot}, task::JoinHandle};
use tonic::{Request, Response, Status, async_trait};

use crate::{raft::Raft, raftproto::{request_vote_server::RequestVote, RequestVoteRequest, RequestVoteResponse}, structs::RaftMsg};

pub struct FlotteService {
    pub raft: Option<JoinHandle<Result<()>>>,
    tx_api: UnboundedSender<RaftMsg>
}

impl FlotteService {
    pub async fn new(id: u64) -> Self {
        let mut peers = Vec::new();
        peers.push("http://[::1]:5000");

        let (tx_api, rx_api) = mpsc::unbounded_channel();

        let raft_handle = Raft::spawn(0, peers, rx_api);
        let this = Self {
            raft: Some(raft_handle),
            tx_api
        };
        this
    }
}

#[async_trait]
impl RequestVote for FlotteService {
    async fn request_vote(
        &self,
        r: tonic::Request<RequestVoteRequest>,
    ) -> Result<Response<RequestVoteResponse>, Status> {
        let (tx, rx) = oneshot::channel();
        let request = r.into_inner();
        self
            .tx_api
            .send(RaftMsg::RequestVote {request, tx} )
            .map_err(|err| println!("{}", err));
        let response = rx.await.map_err(|_| panic!()).and_then(|res|res);
        
        Ok(Response::new(response.unwrap()))
    }
}
