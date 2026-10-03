use crate::network::source::Source;
use bedrock::protocol::PacketDyn;
use std::net::SocketAddr;

pub enum Captured {
    Packet { frame: Vec<u8>, source: Source, addr: SocketAddr },
    Event(NetworkEvent),
}

pub enum NetworkEvent {
    Connected(SocketAddr),
    Disconnected(SocketAddr, String),
    Packet {
        packet: Box<dyn PacketDyn>,
        frame: Vec<u8>,
        source: Source,
        addr: SocketAddr,
    },
    Pong(Box<[u8]>),
    Error(String),
}
