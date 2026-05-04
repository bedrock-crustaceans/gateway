use crate::network::command::NetworkCommand;
use crate::network::event::NetworkEvent;
use std::net::SocketAddr;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub mod event;
pub mod command;
pub mod listener;
pub mod session;
pub mod handler;
pub mod direction;
pub mod login_request;

pub struct Network {
    rx_addr: SocketAddr,
    tx_addr: SocketAddr,
    
    cm_tx: UnboundedSender<NetworkCommand>,
    ev_rx: UnboundedReceiver<NetworkEvent>,
}

impl Network {
    pub fn new(
        rx_addr: SocketAddr,
        tx_addr: SocketAddr,
    ) -> Self {
        let (cm_tx, cm_rx) = unbounded_channel();
        let (ev_tx, ev_rx) = unbounded_channel();
        
        tokio::spawn(async move {
            
        });
        
        Network {
            rx_addr,
            tx_addr,
            
            cm_tx,
            ev_rx
        }
    }
}