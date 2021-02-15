use anyhow::Result;

use tokio::sync::mpsc;
use crate::{raft::{CandidateState, Raft, State}, raftproto::{RequestVoteResponse, RequestVoteRequest, raft_client::RaftClient}};

impl Raft {
    pub async fn handle_vote_request(&mut self, request: RequestVoteRequest) -> Result<RequestVoteResponse> {
        if request.term < self.current_term {
            return Ok(RequestVoteResponse{
                term: self.current_term,
                granted: false
            });
        }

        if request.term > self.current_term {
            self.current_term = request.term;
            self.voted_for = None;
            self.update_election_timeout();
            self.target_state = State::Follower;
            tracing::info!("Transitioning into follower - higher term in VoteRequest");
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
                tracing::info!("Transitioning into follower - higher term in VoteRequest");
                Ok(RequestVoteResponse{
                    term: self.current_term,
                    granted: true
                })
            }
        }
    }
}

impl<'a> CandidateState<'a> {
    pub(super) async fn handle_vote_response(&mut self, response: RequestVoteResponse) -> Result<()> {
        if response.term > self.raft.current_term {
            self.raft.target_state = State::Follower;
            self.raft.current_term = response.term;
            self.raft.update_election_timeout();
            tracing::info!("Transitioning into follower - higher term in RequestVoteResponse");
            return Ok(())
        }
        if response.granted {
            self.votes_granted += 1;
        }
        
        if self.votes_granted >= self.votes_needed {
            tracing::info!("Transitioning into leader");
            self.raft.target_state = State::Leader;
            return Ok(());
        }
        Ok(())
    }

    pub(super) async fn spawn_parallel_vote_requests(&self) -> mpsc::Receiver<RequestVoteResponse> {
        let (tx, rx) = mpsc::channel(self.raft.peers.len());

        for peer in self.raft.peers.clone().into_iter() {
            let request = tonic::Request::new(RequestVoteRequest {
                term: self.raft.current_term,
                candidate: self.raft.id,
                last_log_index: 0,
                last_log_term: 0,
            });
            let mut client = RaftClient::connect(peer.clone()).await;
            let tx_inner = tx.clone();
            match client {
                Ok(mut client) => {
                    let task = async move {
                        match client.request_vote(request).await {
                            Ok(response) => {
                                let _ = tx_inner.send(response.into_inner()).await;
                            }
                            Err(err) => tracing::error!({error=%err, peer=peer.id}, "error while requesting vote from peer"),
                        }
                    };
                    tokio::spawn(task);
                }
                Err(err) => tracing::error!({error=%err, peer=peer.id}, "error while requesting vote from peer"),
            }
        }

        rx
    }
}