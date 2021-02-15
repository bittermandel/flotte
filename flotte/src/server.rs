use std::{collections::{HashSet}, fmt};

use anyhow::Result;

use tokio::{sync::{mpsc::{self, UnboundedSender}, oneshot}, task::JoinHandle};
use tonic::{Request, Response, Status, async_trait};

use crate::{raft::{Peer, Raft}, raftproto::{raft_server, RequestVoteRequest, RequestVoteResponse, AppendEntriesRequest, AppendEntriesResponse}, structs::RaftMsg};

pub struct FlotteService {
    pub raft: Option<JoinHandle<Result<()>>>,
    tx_api: UnboundedSender<RaftMsg>
}

impl FlotteService {
    pub async fn new(id: u64) -> Self {
        let mut peers: HashSet<Peer> = HashSet::new();
        peers.insert(Peer{id: 0, endpoint: "http://localhost:8000".to_string()});
        peers.insert(Peer{id: 1, endpoint: "http://localhost:8001".to_string()});

        let (tx_api, rx_api) = mpsc::unbounded_channel();

        let raft_handle = Raft::spawn(id, peers, rx_api);
        let this = Self {
            raft: Some(raft_handle),
            tx_api
        };
        this
    }
}

#[async_trait]
impl raft_server::Raft for FlotteService {
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

    async fn append_entries(
        &self,
        r: tonic::Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let (tx, rx) = oneshot::channel();
        let request = r.into_inner();
        self
            .tx_api
            .send(RaftMsg::AppendEntries {request, tx} )
            .map_err(|err| println!("{}", err));
        let response = rx.await.map_err(|_| panic!()).and_then(|res|res);
        
        Ok(Response::new(response.unwrap()))
    }
}
