use std::{collections::HashSet, convert::TryFrom, time::Duration};

use anyhow::Result;
use mpsc::UnboundedReceiver;
use rand::{Rng, thread_rng};
use replication::Replication;
use tonic::transport::Endpoint;
use tracing::{Value, field::{Field, Visit}};

use crate::{replication::{self, RaftEvent}, structs::RaftMsg};
use tokio::{sync::mpsc, task::JoinHandle, time::Instant};
use tokio::time::sleep_until;

pub struct Raft {
    pub id: u64,
    pub peers: HashSet<Peer>,
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub target_state: State,
    election_timeout: Option<Instant>,
    rx_api: mpsc::UnboundedReceiver<RaftMsg>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Peer {
    pub id: u64,
    pub endpoint: String
}

impl From<Peer> for Endpoint {
    fn from(p: Peer) -> Endpoint {
        Endpoint::try_from(p.endpoint).unwrap()
    }
}

impl Raft {
    pub fn spawn(id: u64, peers: HashSet<Peer>, rx_api: UnboundedReceiver<RaftMsg>) -> JoinHandle<Result<()>> {
        let this = Self {
            id,
            peers,
            rx_api,
            voted_for: None,
            current_term: 0,
            target_state: State::Follower,
            election_timeout: None,
        };
        tokio::spawn(this.main())
    }

    pub async fn main(mut self) -> Result<()> {
        self.current_term = 0;
        loop {
            match &self.target_state {
                State::Leader => LeaderState::new(&mut self).run().await?,
                State::Follower => FollowerState::new(&mut self).run().await?,
                State::Candidate => CandidateState::new(&mut self).run().await?,
            }
        }
    }

    pub fn get_election_timeout(&mut self) -> Instant {
        match self.election_timeout {
            Some(inst) => inst,
            None => {
                let inst = Instant::now() + Duration::from_millis(thread_rng().gen_range(1500..3000));
                self.election_timeout = Some(inst);
                inst
            }
        }
    }

    pub fn update_election_timeout(&mut self) {
        let inst = Instant::now() + Duration::from_millis(thread_rng().gen_range(1500..3000));
        self.election_timeout = Some(inst);
    }
}

#[derive(PartialEq)]
pub enum State {
    Leader,
    Follower,
    Candidate
}

impl State {
    pub fn is_leader(&self) -> bool {
        matches!(self, Self::Leader)
    }

    pub fn is_follower(&self) -> bool {
        matches!(self, Self::Follower)
    }

    pub fn is_candidate(&self) -> bool {
        matches!(self, Self::Candidate)
    }
}

struct LeaderState<'a> {
    raft: &'a mut Raft,
}

impl<'a> LeaderState<'a> {
    pub fn new(raft: &'a mut Raft) -> Self {
        return Self { raft };
    }

    #[tracing::instrument(level="info", skip(self), fields(id=self.raft.id, raft_state="leader"))]
    pub async fn run(&mut self) -> Result<()> {
        let mut repls: Vec<Replication> = vec![];
        for peer in self.raft.peers.clone().into_iter() {
            if peer.id == self.raft.id {
                continue
            }
            repls.push(Replication::spawn(self.raft.id, Endpoint::from(peer), self.raft.current_term));
        }
        loop {
            if !self.raft.target_state.is_leader() {
                for repl in repls {
                    let _ = repl.repltx.send(RaftEvent::Terminate);
                }
                return Ok(());
            }
            
            tokio::select! {
                Some(msg) = self.raft.rx_api.recv() => match msg {
                    RaftMsg::RequestVote{request, tx} => {
                        let _ = tx.send(self.raft.handle_vote_request(request).await);
                    },
                    RaftMsg::AppendEntries{request, tx} => {
                        let _ = tx.send(self.raft.handle_append_entries_request(request).await);
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

    #[tracing::instrument(level="info", skip(self), fields(id=self.raft.id, raft_state="follower"))]
    pub async fn run(&mut self) -> Result<()> {
        loop {
            if !self.raft.target_state.is_follower() {
                return Ok(());
            }

            let mut election_timeout = sleep_until(self.raft.get_election_timeout());
            tokio::pin!(election_timeout);

            tokio::select! {
                _ = &mut election_timeout => {
                    tracing::info!("Transitioning into Candidate - election timer ran out");
                    self.raft.target_state = State::Candidate;
                }
                Some(msg) = self.raft.rx_api.recv() => match msg {
                    RaftMsg::RequestVote{request, tx} => {
                        let _ = tx.send(self.raft.handle_vote_request(request).await);
                    },
                    RaftMsg::AppendEntries{request, tx} => {
                        let _ = tx.send(self.raft.handle_append_entries_request(request).await);
                    }
                }
            }
        }
    }
}

pub struct CandidateState<'a> {
    pub raft: &'a mut Raft,
    pub votes_granted: u64,
    pub votes_needed: u64
}

impl<'a> CandidateState<'a> {
    pub fn new(raft: &'a mut Raft) -> Self  {
        Self { 
            raft,
            votes_granted: 0,
            votes_needed: 0,
        }
    }

    #[tracing::instrument(level="info", skip(self), fields(id=self.raft.id, raft_state="candidate"))]
    pub async fn run(&mut self) -> Result<()> {
        loop {
            if !self.raft.target_state.is_candidate() {
                return Ok(());
            }

            self.votes_granted = 1;
            self.votes_needed = ((self.raft.peers.len() / 2) + 1) as u64;

            self.raft.update_election_timeout();
            self.raft.current_term += 1;
            self.raft.voted_for = Some(self.raft.id);
            self.raft.election_timeout = None;

            let mut pending_votes = self.spawn_parallel_vote_requests().await;

            let election_timeout = sleep_until(self.raft.get_election_timeout());
            tokio::pin!(election_timeout);

            loop {
                if !self.raft.target_state.is_candidate() {
                    return Ok(());
                }

                tokio::select! {
                    _ = &mut election_timeout => break,
                    Some (response) = pending_votes.recv() => self.handle_vote_response(response).await?,
                    Some(msg) = self.raft.rx_api.recv() => match msg {
                        RaftMsg::RequestVote{request, tx} => {
                            let _ = tx.send(self.raft.handle_vote_request(request).await);
                        },
                        RaftMsg::AppendEntries{request, tx} => {
                            let _ = tx.send(self.raft.handle_append_entries_request(request).await);
                        }
                    }
                }
            }
        }
    }
}