use anyhow::Result;

use crate::raft::{Raft, State};
use crate::raftproto::{AppendEntriesResponse, AppendEntriesRequest};

impl Raft {
    
    pub async fn handle_append_entries_request(&mut self, request: AppendEntriesRequest) -> Result<AppendEntriesResponse> {
        if request.term < self.current_term {
            return Ok(AppendEntriesResponse{
                success: false,
                term: self.current_term,
                index: 0,
                commit_index: 0
            });
        }
       
        self.update_election_timeout();

        if self.current_term != request.term {
            self.current_term = request.term;
        }

        if self.target_state != State::Follower {
            self.target_state = State::Follower;
        }

        if request.entries.is_empty() {
            return Ok(AppendEntriesResponse{
                success: true,
                term: self.current_term,
                index: 0,
                commit_index: 0
            });
        }

        return Ok(AppendEntriesResponse{
            success: true,
            term: self.current_term,
            index: 0,
            commit_index: 0
        });
    }
}