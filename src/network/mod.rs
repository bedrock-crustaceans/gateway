use crate::network::command::NetworkCommand;
use crate::network::event::NetworkEvent;
use crate::BedrockProtocol;
use bedrock::network::listener::Listener;
use bedrock::protocol::ProtoVersion;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::interval;
use crate::network::direction::Direction;
use crate::network::session::Session;

pub mod event;
pub mod command;
pub mod listener;
pub mod session;
pub mod session_state;
pub mod direction;
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
            let mut listener = Listener::new_raknet(
                rx_addr,
                "Gateway".to_string(),
                "https://bedrock-crustaceans.org/gateway".to_string(),
                BedrockProtocol::GAME_VERSION.to_string(),
                BedrockProtocol::PROTOCOL_VERSION,
                BedrockProtocol::RAKNET_VERSION,
                0,
                0,
                false,
            ).await.unwrap();

            listener.start().await.unwrap();
            
            let mut tick = interval(Duration::from_millis(20));
            
            let mut sessions = Vec::new();

            loop {
                tokio::select! {
                    Some(cmd) = cm_rx.recv() => {
                        match cmd {
                            NetworkCommand::Stop => break
                        }
                    },
                    Ok(conn) = listener.accept::<BedrockProtocol>() => {
                        let session = Session::new(conn, Direction::Upstream);
                        sessions.push(session);
                    }
                    _ = tick.tick() => {
                        for session in sessions.iter_mut() {
                            session.tick();
                            
                            while let Some(packet) = session.recv() {
                                session.handle(&packet);
                                
                                ev_tx.send(NetworkEvent::Packet {
                                    packet,
                                    addr: session.addr,
                                    direction: session.direction
                                }).unwrap();
                            }
                        }
                    }
                    else => break,
                }
            }
            
            listener.stop().await.unwrap();
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