use crate::network::source::Source;
use crate::BedrockProtocol;
use std::net::SocketAddr;

pub enum NetworkEvent {
    Started,
    Stopped,
    Packet {
        packet: BedrockProtocol,
        source: Source,
        addr: SocketAddr,
    }
}