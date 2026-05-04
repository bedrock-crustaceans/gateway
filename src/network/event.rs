use crate::network::direction::Direction;
use crate::BedrockProtocol;
use std::net::SocketAddr;

pub enum NetworkEvent {
    Started,
    Stopped,
    Packet {
        packet: BedrockProtocol,
        addr: SocketAddr,
        direction: Direction,
    }
}