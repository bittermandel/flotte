use std::time::Duration;

use tokio::{sync::mpsc::{self, UnboundedSender}, task::JoinHandle, time::{Interval, interval}};
use tonic::transport::Endpoint;
use crate::raftproto::{AppendEntriesRequest, raft_client::RaftClient};


pub struct Replication {
    handle: JoinHandle<()>,
    pub repltx: mpsc::UnboundedSender<RaftEvent>,
}

impl Replication {
    pub fn spawn(id: u64, target: Endpoint, term: u64) -> Self {
        ReplicationInner::spawn(id, target, term)
    }
}
struct ReplicationInner {
    id: u64,
    target: Endpoint,
    term: u64,
    heartbeat: Interval,
    raftrx: mpsc::UnboundedReceiver<RaftEvent>
}

impl ReplicationInner {
    pub fn spawn(id: u64, target: Endpoint, term: u64) -> Replication {
        let (raftrx_tx, raftrx) = mpsc::unbounded_channel();
        
        let mut this = Self {
            id,
            target,
            term,
            heartbeat: interval(Duration::from_millis(200)),
            raftrx
        };

        let handle = tokio::spawn(this.main());
        return Replication { handle, repltx: raftrx_tx }
    }

    async fn main(mut self) {
        self.send_append_entries().await;

        loop {
            tokio::select! {
                _ = self.heartbeat.tick() => self.send_append_entries().await,
                event = self.raftrx.recv() => match event {
                    Some(RaftEvent::Terminate) => {
                        return;
                    }
                    None => return
                }
            }
        }
    }

    async fn send_append_entries(&mut self) {
        let request = AppendEntriesRequest{
            entries: vec![],
            leader_id: self.id,
            term: self.term
        };

        let request = tonic::Request::new(request);
        let mut client = RaftClient::connect(self.target.clone()).await;
        match client {
            Ok(mut client) => {
                let task = async move {
                    match client.append_entries(request).await {
                        Ok(response) => {}
                        Err(err) => tracing::error!({error=%err}, "error while append entry"),
                    }
                };
                tokio::spawn(task);
            }
            Err(err) => tracing::error!({error=%err}, "error while append entry"),
        }
    }
}

pub enum RaftEvent {
    Terminate
}