//! Simple P2P networking layer (TCP + gossip)

pub mod protocol;
pub mod peer;
pub mod gossip;

pub use protocol::{NetworkMessage, MessageCodec};
pub use peer::{PeerManager, PeerInfo};
pub use gossip::GossipLayer;
