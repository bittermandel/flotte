use std::{error::Error};
use anyhow::Result;

use futures::{FutureExt, executor::block_on};
use raftproto::{request_vote_client::RequestVoteClient};
use crate::{raftproto};

pub struct Raft {
    id: u64,
    peers: Vec<&'static str>,
    term: u64,
    target_state: State
}

impl Raft {
    pub fn spawn(id: u64, peers: Vec<&'static str>) {
        let this = Self {
            id,
            peers,
            term: 0,
            target_state: State::Leader
        };
        tokio::spawn(this.main());
    }

    pub async fn request_vote(&self) -> Result<()> {
        for &peer in &self.peers {
            let request = tonic::Request::new(raftproto::RequestVoteRequest {
                term: self.term,
                candidate: self.id,
                last_log_index: 0,
                last_log_term: 0,
            });
            let mut client = RequestVoteClient::connect(peer).await;
            match client {
                Ok(mut client) => {
                    println!("TEST");
                    let task = async move {
                        match client.request_vote(request).await {
                            Ok(_) => println!("All OK"),
                            Err(err) => panic!(err)
                        }
                    };
                    tokio::spawn(task);
                }
                Err(err) => println!("TEST")
            }
        }
        return Ok(())
    }
    async fn main(mut self) -> Result<()> {
        self.term = 0;
        loop {
            match &self.target_state {
                State::Leader => {
                    LeaderState::new(&mut self).run().await?
                }
                State::Follower => {
                    FollowerState::new(&mut self).run().await?
                }
            }
        }
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
    raft: &'a mut Raft
}

impl<'a> LeaderState<'a> {
    pub fn new(raft: &'a mut Raft) -> Self {
        return Self {
            raft
        }
    }

    pub async fn run(mut self) -> Result<()> {

        loop {
            println!("I AM LEADER");
            self.raft.request_vote().await;
        }
    }
}

struct FollowerState<'a> {
    raft: &'a mut Raft
}

impl<'a> FollowerState<'a> {
    pub fn new(raft: &'a mut Raft) -> Self {
        return Self {
            raft
        }
    }

    pub async fn run(mut self) -> Result<()>{
        println!("I AM FOLLOWER");
        loop {
        }
    }
}