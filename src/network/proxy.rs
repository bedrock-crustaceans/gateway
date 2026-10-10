use crate::network::codec::{decode, encode, packet_id, read_frames, write_frames, Frame};
use crate::network::event::{Captured, NetworkEvent};
use crate::network::login::ProxyKeys;
use crate::network::source::Source;
use crate::network::Transport;
use crate::{BedrockConnection, BedrockProtocol};
use bedrock::network::compression::Compression;
use bedrock::network::error::CompressionError;
use bedrock::network::tokio::transport::{NetherNetConnection, TransportLayerConnection};
use bedrock::network::tokio::Connection;
use bedrock::protocol::v662::packets::{ClientToServerHandshakePacket, LoginPacket, NetworkSettingsPacket, ServerToClientHandshakePacket};
use bedrock::protocol::{Packet, ProtoVersion};
use raknet_tokio::prelude::RakClient;
use std::net::SocketAddr;
use tokio::sync::mpsc::UnboundedSender;

const INTERCEPTED: [u16; 4] = [
    LoginPacket::ID,
    ServerToClientHandshakePacket::ID,
    ClientToServerHandshakePacket::ID,
    NetworkSettingsPacket::<BedrockProtocol>::ID,
];

pub fn negotiated_compression(settings: &NetworkSettingsPacket<BedrockProtocol>) -> Result<Compression, CompressionError> {
    Compression::from_network_settings(settings.compression_algorithm.clone() as u16, settings.compression_threshold)
}

fn intercept(frame: &[u8]) -> Option<BedrockProtocol> {
    packet_id(frame).filter(|id| INTERCEPTED.contains(id))?;
    decode(frame)
}

struct Proxy {
    addr: SocketAddr,
    client: BedrockConnection,
    server: BedrockConnection,
    keys: ProxyKeys,
    capture: UnboundedSender<Captured>,
}

pub async fn run(transport: Transport, client: BedrockConnection, target: SocketAddr, capture: UnboundedSender<Captured>) {
    let addr = client.get_socket_addr();
    _ = capture.send(Captured::Event(NetworkEvent::Connected(addr)));

    let mut rak_client = None;
    let upstream = match transport {
        Transport::RakNet => {
            let mut raknet = RakClient::new(|conf| conf.protocol = BedrockProtocol::RAKNET_VERSION);
            let session = match raknet.start().await {
                Ok(()) => raknet.connect(target).await.map(|session| TransportLayerConnection::RakNet(session.into())).map_err(|err| format!("Failed to connect to {target}: {err:?}")),
                Err(err) => Err(format!("Failed to start upstream client: {err:?}")),
            };
            rak_client = Some(raknet);
            session
        }
        Transport::NetherNet => NetherNetConnection::connect_http(target)
            .await
            .map(TransportLayerConnection::NetherNet)
            .map_err(|err| format!("Failed to connect to {target}: {err}")),
    };

    let reason = match upstream {
        Ok(upstream) => {
            let mut proxy = Proxy {
                addr,
                client,
                server: Connection::from_transport_conn(upstream),
                keys: ProxyKeys::generate(),
                capture: capture.clone(),
            };
            let reason = proxy.relay().await;
            proxy.client.close().await;
            proxy.server.close().await;
            reason
        }
        Err(reason) => {
            client.close().await;
            reason
        }
    };

    if let Some(mut raknet) = rak_client {
        raknet.stop().await;
    }
    _ = capture.send(Captured::Event(NetworkEvent::Disconnected(addr, reason)));
}

impl Proxy {
    async fn relay(&mut self) -> String {
        loop {
            let result = tokio::select! {
                frames = read_frames(&mut self.client) => match frames {
                    Ok(frames) => self.handle_client(frames).await,
                    Err(err) => Err(format!("Client: {err}")),
                },
                frames = read_frames(&mut self.server) => match frames {
                    Ok(frames) => self.handle_server(frames).await,
                    Err(err) => Err(format!("Server: {err}")),
                },
            };

            if let Err(reason) = result {
                return reason;
            }
        }
    }

    fn log(&self, source: Source, frame: &Frame) {
        _ = self.capture.send(Captured::Packet {
            frame: frame.clone(),
            source,
            addr: self.addr,
        });
    }

    async fn handle_client(&mut self, frames: Vec<Frame>) -> Result<(), String> {
        let mut out = Vec::with_capacity(frames.len());

        for frame in frames {
            self.log(Source::Client, &frame);

            match intercept(&frame) {
                Some(BedrockProtocol::LoginPacket(mut login)) => {
                    login.connection_request = self
                        .keys
                        .forge_login(&login.connection_request)
                        .ok_or("Failed to rewrite login request")?;
                    out.push(encode(&BedrockProtocol::LoginPacket(login)));
                }
                Some(BedrockProtocol::ClientToServerHandshakePacket(_)) => {}
                _ => out.push(frame),
            }
        }

        write_frames(&mut self.server, &out).await
    }

    async fn handle_server(&mut self, frames: Vec<Frame>) -> Result<(), String> {
        let mut out = Vec::with_capacity(frames.len());
        let mut compression = None;

        for frame in frames {
            self.log(Source::Server, &frame);

            match intercept(&frame) {
                Some(BedrockProtocol::ServerToClientHandshakePacket(handshake)) => {
                    self.server.encryption = Some(
                        self.keys
                            .server_encryption(&handshake.handshake_web_token)
                            .ok_or("Invalid server handshake")?,
                    );
                    let reply = BedrockProtocol::ClientToServerHandshakePacket(Box::new(ClientToServerHandshakePacket {}));
                    write_frames(&mut self.server, &[encode(&reply)]).await?;
                }
                Some(BedrockProtocol::NetworkSettingsPacket(settings)) => {
                    compression = Some(negotiated_compression(&settings).map_err(|err| err.to_string())?);
                    out.push(frame);
                }
                _ => out.push(frame),
            }
        }

        write_frames(&mut self.client, &out).await?;

        if let Some(compression) = compression {
            self.client.compression = Some(compression.clone());
            self.server.compression = Some(compression);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock::protocol::v662::enums::PacketCompressionAlgorithm;
    use bedrock::protocol::v662::packets::RequestNetworkSettingsPacket;

    #[test]
    fn only_rewritten_packets_are_decoded_on_the_relay_path() {
        let login = encode(&BedrockProtocol::LoginPacket(Box::new(LoginPacket {
            client_network_version: 2193,
            connection_request: vec![],
        })));
        let other = encode(&BedrockProtocol::RequestNetworkSettingsPacket(Box::new(RequestNetworkSettingsPacket {
            client_network_version: 2193,
        })));

        assert!(matches!(intercept(&login), Some(BedrockProtocol::LoginPacket(_))));
        assert!(intercept(&other).is_none());
    }

    fn settings(compression_algorithm: PacketCompressionAlgorithm, compression_threshold: u16) -> NetworkSettingsPacket<BedrockProtocol> {
        NetworkSettingsPacket {
            compression_threshold,
            compression_algorithm,
            client_throttle_enabled: false,
            client_throttle_threshold: 0,
            client_throttle_scalar: 0.,
        }
    }

    #[test]
    fn compression_follows_the_server_network_settings() {
        assert!(matches!(
            negotiated_compression(&settings(PacketCompressionAlgorithm::ZLib, 256)),
            Ok(Compression::Zlib { threshold: 256, .. })
        ));
        assert!(matches!(
            negotiated_compression(&settings(PacketCompressionAlgorithm::Snappy, 1)),
            Ok(Compression::Snappy { threshold: 1 })
        ));
        assert!(matches!(negotiated_compression(&settings(PacketCompressionAlgorithm::None, 0)), Ok(Compression::None)));
    }
}
