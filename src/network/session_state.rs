use bedrock::network::compression::Compression;
use bedrock::protocol::ProtoVersion;
use bedrock::protocol::v662::enums::{PacketCompressionAlgorithm, PlayStatus};
use bedrock::protocol::v662::packets::{NetworkSettingsPacket, PlayStatusPacket};
use crate::BedrockProtocol;
use crate::network::source::Source;
use crate::network::session::Session;

pub enum SessionState {
    Login,
    Play
}

impl Session {
    pub fn handle(&mut self, packet: &BedrockProtocol) {
        match self.state {
            SessionState::Login => self.handle_login(packet),
            SessionState::Play => self.handle_play(packet),
        }
    }
    
    pub fn handle_login(&mut self, packet: &BedrockProtocol) {
        match (self.source, packet) {
            (Source::Client, BedrockProtocol::RequestNetworkSettingsPacket(packet)) => {
                let protocol = packet.client_network_version as u32;
                if protocol != BedrockProtocol::PROTOCOL_VERSION {
                    self.send_immediate(
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

                self.send_immediate(BedrockProtocol::NetworkSettingsPacket(NetworkSettingsPacket {
                    compression_threshold: 1,
                    compression_algorithm: PacketCompressionAlgorithm::None,
                    client_throttle_enabled: false,
                    client_throttle_threshold: 0,
                    client_throttle_scalar: 0.0,
                }.into()));

                self.set_compression(Some(Compression::None));
            },
            (Source::Client, BedrockProtocol::LoginPacket(packet)) => {
                
            }
            _ => {}
        }
    }
    
    pub fn handle_play(&self, packet: &BedrockProtocol) {
        
    }
}