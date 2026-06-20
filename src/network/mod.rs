use crate::network::command::NetworkCommand;
use crate::network::event::NetworkEvent;
use crate::network::session::Session;
use crate::network::source::Source;
use crate::BedrockProtocol;
use bedrock::network::connection::Connection;
use bedrock::network::info::MINECRAFT_EDITION_MOTD;
use bedrock::network::motd::BedrockMOTD;
use bedrock::network::transport::TransportLayerConnection;
use bedrock::protocol::ProtoVersion;
use raknet_tokio::prelude::{RakClient, RakServer};
use rand::random;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::interval;

pub mod event;
pub mod command;
pub mod listener;
pub mod session;
pub mod session_state;
pub mod source;
pub mod login_request;

pub struct Network {
    pub rx_addr: SocketAddr,
    pub tx_addr: SocketAddr,
    
    pub cm_tx: UnboundedSender<NetworkCommand>,
    pub ev_rx: UnboundedReceiver<NetworkEvent>,
}

impl Network {
    pub fn new(
        rx_addr: SocketAddr,
        tx_addr: SocketAddr,
    ) -> Self {
        let (cm_tx, mut cm_rx) = unbounded_channel();
        let (ev_tx, ev_rx) = unbounded_channel();
        
        tokio::spawn(async move {
            let guid: u64 = random::<u64>();

            let mut server = RakServer::new(rx_addr, |conf| {
                conf.guid = guid;
                conf.protocols = Box::new([BedrockProtocol::RAKNET_VERSION]);
                conf.message = BedrockMOTD {
                    edition: MINECRAFT_EDITION_MOTD.to_owned(),
                    version: BedrockProtocol::GAME_VERSION.to_string(),
                    name: "Gateway".to_string(),
                    sub_name: "https://bedrock-crustaceans.org/gateway".to_string(),
                    player_max: 0,
                    player_count: 0,
                    protocol: BedrockProtocol::PROTOCOL_VERSION,
                    guid,
                    game_mode: "Survival".to_string(),
                    port_v4: Some(rx_addr.port()),
                    port_v6: Some(rx_addr.port()),
                    nintendo_limited: Some(false),
                }.into()
            });

            server.start().await.unwrap();
            
            let mut client = RakClient::new(|_| {});
            
            client.start().await.unwrap();
            
            let mut tick = interval(Duration::from_millis(20));
            
            let mut sessions = Vec::new();

            loop {
                tokio::select! {
                    Some(cmd) = cm_rx.recv() => {
                        match cmd {
                            NetworkCommand::Stop => break
                        }
                    },
                    Ok(conn) = server.accept() => {
                        let conn = Connection::from_transport_conn(TransportLayerConnection::RakNet(conn));
                        
                        let session = Session::new(conn, Source::Client);
                        sessions.push(session);
                    }
                    Ok((msg, _)) = client.ping(tx_addr) => {
                        ev_tx.send(NetworkEvent::Pong(msg)).unwrap();
                    }
                    _ = tick.tick() => {
                        for session in sessions.iter_mut() {
                            session.tick();
                            
                            while let Some(packet) = session.recv() {
                                session.handle(&packet);
                                
                                ev_tx.send(NetworkEvent::Packet {
                                    packet,
                                    addr: session.addr,
                                    source: session.source
                                }).unwrap();
                            }
                        }
                    }
                    else => break,
                }
            }
            
            server.stop();
        });
        
        Network {
            rx_addr,
            tx_addr,
            
            cm_tx,
            ev_rx
        }
    }
    
    pub fn close(&self) {
        self.cm_tx.send(NetworkCommand::Stop).unwrap();
    }
}