use bedrock::network::compression::Compression;
use bedrock::protocol::ProtoVersion;
use bedrock::protocol::v662::enums::{PacketCompressionAlgorithm, PlayStatus};
use bedrock::protocol::v662::packets::{NetworkSettingsPacket, PlayStatusPacket};
use crate::BedrockProtocol;
use crate::network::direction::Direction;
use crate::network::session::Session;

pub enum PacketHandler {
    Login(Direction),
    Play
}

impl PacketHandler {
    pub fn handle(&self, session: &mut Session, packet: BedrockProtocol) {
        match self {
            PacketHandler::Login(direction) => self.handle_login(session, packet, direction),
            PacketHandler::Play => self.handle_play(packet),
        }
    }
    
    pub fn handle_login(&self, session: &mut Session, packet: BedrockProtocol, direction: &Direction) {
        match (direction, packet) {
            (Direction::Upstream, BedrockProtocol::RequestNetworkSettingsPacket(packet)) => {
                let protocol = packet.client_network_version as u32;
                if protocol != BedrockProtocol::PROTOCOL_VERSION {
                    session.send_immediate(
                        BedrockProtocol::PlayStatusPacket(PlayStatusPacket {
                            status: if protocol < BedrockProtocol::PROTOCOL_VERSION {
                                PlayStatus::LoginFailedClientOld
                            } else {
                                PlayStatus::LoginFailedServerOld
                            },
                        }.into())
                    );
                    return;
                }
                
                session.send_immediate(BedrockProtocol::NetworkSettingsPacket(NetworkSettingsPacket {
                    compression_threshold: 1,
                    compression_algorithm: PacketCompressionAlgorithm::None,
                    client_throttle_enabled: false,
                    client_throttle_threshold: 0,
                    client_throttle_scalar: 0.0,
                }.into()));

                session.set_compression(Some(Compression::None));
            },
            (Direction::Upstream, BedrockProtocol::LoginPacket(packet)) => {
                
            }
            _ => {}
        }
    }
    
    pub fn handle_play(&self, packet: BedrockProtocol) {
        
    }
}