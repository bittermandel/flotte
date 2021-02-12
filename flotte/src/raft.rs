use anyhow::Result;
use mpsc::UnboundedReceiver;
use tonic::Request;
use std::{error::Error, time::{Duration, Instant}};
use raftproto::{RequestVoteResponse, RequestVoteRequest};

use crate::{raftproto, structs::RaftMsg};
use raftproto::request_vote_client::RequestVoteClient;
use tokio::{sync::mpsc, task::JoinHandle, time::sleep};

pub struct Raft {
    id: u64,
    peers: Vec<&'static str>,
    current_term: u64,
    voted_for: Option<u64>,
    target_state: State,
    rx_api: mpsc::UnboundedReceiver<RaftMsg>,
}
impl Raft {
    async fn handle_vote_request(&mut self, request: RequestVoteRequest) -> Result<RequestVoteResponse> {
        if request.term < self.current_term {
            return Ok(RequestVoteResponse{
                term: self.current_term,
                granted: false
            });
        }

        if request.term > self.current_term {
            self.current_term = request.term;
            self.voted_for = None;
            self.target_state = State::Follower;
        }

        match self.voted_for {
            Some(candidate_id) if candidate_id == request.candidate => Ok(RequestVoteResponse{
                term: self.current_term,
                granted: true
            }),
            Some(_) => Ok(RequestVoteResponse{
                term: self.current_term,
                granted: false
            }),
            None => {
                self.voted_for = Some(request.candidate);
                self.target_state = State::Follower;
                Ok(RequestVoteResponse{
                    term: self.current_term,
                    granted: true
                })
            }
        }
    }
}
impl Raft {
    pub fn spawn(id: u64, peers: Vec<&'static str>, rx_api: UnboundedReceiver<RaftMsg>) -> JoinHandle<Result<()>> {
        let this = Self {
            id,
            peers,
            rx_api,
            voted_for: None,
            current_term: 0,
            target_state: State::Follower,
        };
        tokio::spawn(this.main())
    }

    pub async fn main(mut self) -> Result<()> {
        self.current_term = 0;
        loop {
            match &self.target_state {
                State::Leader => LeaderState::new(&mut self).run().await?,
                State::Follower => FollowerState::new(&mut self).run().await?,
            }
        }
    }

    pub async fn request_vote(&self) -> Result<()> {
        for &peer in &self.peers {
            let request = tonic::Request::new(raftproto::RequestVoteRequest {
                term: self.current_term,
                candidate: self.id,
                last_log_index: 0,
                last_log_term: 0,
            });
            let mut client = RequestVoteClient::connect(peer).await;
            match client {
                Ok(mut client) => {
                    let task = async move {
                        match client.request_vote(request).await {
                            Ok(_) => println!("All OK"),
                            Err(err) => panic!(err),
                        }
                    };
                    tokio::spawn(task);
                }
                Err(err) => println!("{}", err),
            }
        }
        return Ok(());
    }
}

enum State {
    Leader,
    Follower,
}

impl State {
    pub fn is_leader(&self) -> bool {
        matches!(self, Self::Leader)
    }

    pub fn is_follower(&self) -> bool {
        matches!(self, Self::Follower)
    }
}

struct LeaderState<'a> {
    raft: &'a mut Raft,
}

impl<'a> LeaderState<'a> {
    pub fn new(raft: &'a mut Raft) -> Self {
        return Self { raft };
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            if !self.raft.target_state.is_leader() {
                return Ok(());
            }

            tokio::select! {
                Some(msg) = self.raft.rx_api.recv() => match msg {
                    RaftMsg::RequestVote{request, tx} => {
                        let _ = tx.send(self.raft.handle_vote_request(request).await);
                    }
                }
            }
        }
    }
}

struct FollowerState<'a> {
    raft: &'a mut Raft,
}

impl<'a> FollowerState<'a> {
    pub fn new(raft: &'a mut Raft) -> Self {
        return Self { raft };
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            if !self.raft.target_state.is_follower() {
                return Ok(());
            }

            tokio::select! {
                Some(msg) = self.raft.rx_api.recv() => match msg {
                    RaftMsg::RequestVote{request, tx} => {
                        let mut granted = false;
                        if request.term > self.raft.current_term {
                            self.raft.current_term = request.term;
                            self.raft.voted_for = None;
                        }
                        if self.raft.voted_for.is_none() || self.raft.voted_for == Some(request.candidate) {
                            self.raft.voted_for = Some(request.candidate);
                            granted = true;
                        }
                        let _ = tx.send(Ok(raftproto::RequestVoteResponse{ term: self.raft.current_term, granted: granted}));
                    }
                }
            }
        }
    }
}
