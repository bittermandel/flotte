use tokio::sync::oneshot;

use crate::raftproto;
use anyhow::Result;

#[derive(Debug)]
pub struct VoteRequest {
    pub term: u64,
    pub candidate_id: u64,
    pub last_log_index: u64,
    pub last_log_term: u64   
}

#[derive(Debug)]
pub struct VoteResponse {
    pub term: u64,
    pub vote_granted: bool
}

pub enum RaftMsg {
    RequestVote {
        request: raftproto::RequestVoteRequest,
        tx: oneshot::Sender<Result<raftproto::RequestVoteResponse>>
    },
    AppendEntries {
        request: raftproto::AppendEntriesRequest,
        tx: oneshot::Sender<Result<raftproto::AppendEntriesResponse>>
    }
}