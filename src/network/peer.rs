//! Peer connection management

use super::protocol::{MessageCodec, NetworkMessage};
use anyhow::Result;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_util::codec::Framed;
use tracing::{info, warn, error};

/// Peer information
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub connected: bool,
}

/// Peer manager
pub struct PeerManager {
    peers: Arc<RwLock<HashMap<SocketAddr, PeerInfo>>>,
    tx: mpsc::UnboundedSender<(SocketAddr, NetworkMessage)>,
    rx: mpsc::UnboundedReceiver<(SocketAddr, NetworkMessage)>,
}

impl PeerManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            tx,
            rx,
        }
    }

    /// Start TCP listener
    pub async fn listen(&self, addr: SocketAddr) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        info!("Listening on {}", addr);

        let peers = self.peers.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((socket, peer_addr)) => {
                        info!("New connection from {}", peer_addr);
                        let tx = tx.clone();
                        tokio::spawn(Self::handle_connection(socket, peer_addr, tx));
                    }
                    Err(e) => {
                        error!("Accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Connect to peer
    pub async fn connect(&self, addr: SocketAddr) -> Result<()> {
        let socket = TcpStream::connect(addr).await?;
        info!("Connected to {}", addr);

        let tx = self.tx.clone();
        tokio::spawn(Self::handle_connection(socket, addr, tx));

        Ok(())
    }

    /// Handle peer connection
    async fn handle_connection(
        socket: TcpStream,
        peer_addr: SocketAddr,
        tx: mpsc::UnboundedSender<(SocketAddr, NetworkMessage)>,
    ) {
        let mut framed = Framed::new(socket, MessageCodec::new());

        while let Some(result) = framed.next().await {
            match result {
                Ok(msg) => {
                    if tx.send((peer_addr, msg)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    warn!("Connection error from {}: {}", peer_addr, e);
                    break;
                }
            }
        }

        info!("Disconnected from {}", peer_addr);
    }

    /// Broadcast message to all peers
    pub async fn broadcast(&self, msg: NetworkMessage) -> Result<()> {
        let peers = self.peers.read().await;
        info!("Broadcasting to {} peers", peers.len());
        
        // TODO: Send via stored peer connections
        Ok(())
    }

    /// Receive next message
    pub async fn recv(&mut self) -> Option<(SocketAddr, NetworkMessage)> {
        self.rx.recv().await
    }
}

impl Default for PeerManager {
    fn default() -> Self {
        Self::new()
    }
}
