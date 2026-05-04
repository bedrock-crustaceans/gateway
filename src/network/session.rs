use crate::network::direction::Direction;
use crate::network::session_state::SessionState;
use crate::{BedrockConnection, BedrockProtocol};
use bedrock::network::compression::Compression;
use bedrock::network::encryption::Encryption;
use std::mem::take;
use std::net::SocketAddr;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub enum ConnectionEvent {
    Send(Vec<BedrockProtocol>),
    SetCompression(Option<Compression>),
    SetEncryption(Option<Box<Encryption>>),
}

pub struct Session {
    pub addr: SocketAddr,
    pub state: SessionState,
    pub direction: Direction,
    
    out_q: Vec<BedrockProtocol>,
    inc_rx: UnboundedReceiver<BedrockProtocol>,
    conn_tx: UnboundedSender<ConnectionEvent>,
}

impl Session {
    pub fn new(conn: BedrockConnection, direction: Direction) -> Self {
        let (inc_tx, inc_rx) = tokio::sync::mpsc::unbounded_channel::<BedrockProtocol>();
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::unbounded_channel::<ConnectionEvent>();
        
        let addr = *conn.get_socket_addr();
        
        let mut conn = conn;
        tokio::spawn(async move {
            'l: loop {
                if conn.is_closed().await { break 'l; }
                
                tokio::select! {
                    recv = conn.recv() => {
                        match recv {
                            Ok(packets) => {
                                for packet in packets {
                                    if inc_tx.send(packet).is_err() {
                                        break 'l;
                                    }
                                }
                            }
                            Err(_) => break 'l
                        }
                    },
                    Some(event) = conn_rx.recv() => {
                        match event {
                            ConnectionEvent::Send(packets) => {
                                if !packets.is_empty()
                                    && let Err(_) = conn.send(&packets).await
                                {
                                    break 'l;
                                }
                            }
                            ConnectionEvent::SetCompression(compression) => {
                                conn.compression = compression;
                            }
                            ConnectionEvent::SetEncryption(encryption) => {
                                conn.encryption = encryption.map(|b| *b);
                            }
                        }
                    },
                    else => break
                }
            }
            
            conn.close().await;
        });
        
        Session {
            addr,
            state: SessionState::Login,
            direction,
            
            out_q: Vec::new(),
            inc_rx,
            conn_tx
        }
    }


    pub fn send_immediate(&self, packet: BedrockProtocol) {
        _ = self.conn_tx.send(ConnectionEvent::Send(vec![packet]));
    }

    pub fn send(&mut self, packet: BedrockProtocol) {
        self.out_q.push(packet);
    }

    pub fn tick(&mut self) {
        let out = take(&mut self.out_q);
        if !out.is_empty() {
            _ = self.conn_tx.send(ConnectionEvent::Send(out));
        }
    }

    pub fn recv(&mut self) -> Option<BedrockProtocol> {
        self.inc_rx.try_recv().ok()
    }

    pub fn set_compression(&self, compression: Option<Compression>) {
        _ = self.conn_tx.send(ConnectionEvent::SetCompression(compression));
    }

    pub fn set_encryption(&self, encryption: Option<Encryption>) {
        _ = self.conn_tx.send(ConnectionEvent::SetEncryption(encryption.map(Box::new)));
    }
}