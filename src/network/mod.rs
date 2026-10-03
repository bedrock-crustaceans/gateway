use crate::network::command::NetworkCommand;
use crate::network::codec::decode_dyn;
use crate::network::event::{Captured, NetworkEvent};
use crate::BedrockProtocol;
use bedrock::network::info::MINECRAFT_EDITION_MOTD;
use bedrock::network::motd::BedrockMOTD;
use bedrock::protocol::ProtoVersion;
use raknet_tokio::prelude::{RakClient, RakServer};
use rand::random;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;
use tokio::time::interval;

pub mod codec;
pub mod command;
pub mod event;
pub mod login;
pub mod proxy;
pub mod source;

pub struct Network {
    pub rx_addr: SocketAddr,
    pub tx_addr: SocketAddr,

    pub cm_tx: UnboundedSender<NetworkCommand>,
    pub ev_rx: UnboundedReceiver<NetworkEvent>,
}

impl Network {
    pub fn new(rx_addr: SocketAddr, tx_addr: SocketAddr) -> Self {
        let (cm_tx, cm_rx) = unbounded_channel();
        let (ev_tx, ev_rx) = unbounded_channel();

        tokio::spawn(async move {
            if let Err(err) = run(rx_addr, tx_addr, cm_rx, ev_tx.clone()).await {
                _ = ev_tx.send(NetworkEvent::Error(err));
            }
        });

        Network {
            rx_addr,
            tx_addr,

            cm_tx,
            ev_rx,
        }
    }

    pub fn close(&self) {
        _ = self.cm_tx.send(NetworkCommand::Stop);
    }
}

async fn run(
    rx_addr: SocketAddr,
    tx_addr: SocketAddr,
    mut cm_rx: UnboundedReceiver<NetworkCommand>,
    ev_tx: UnboundedSender<NetworkEvent>,
) -> Result<(), String> {
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
        }
        .into()
    });
    server.start().await.map_err(|e| format!("Failed to bind {rx_addr}: {e:?}"))?;

    let mut pinger = RakClient::new(|conf| conf.protocol = BedrockProtocol::RAKNET_VERSION);
    pinger.start().await.map_err(|e| format!("Failed to start client: {e:?}"))?;

    let capture = spawn_decoder(ev_tx.clone());
    let mut tasks = JoinSet::new();

    let ping_tx = ev_tx.clone();
    tasks.spawn(async move {
        let mut tick = interval(Duration::from_secs(3));
        loop {
            tick.tick().await;
            if let Ok((msg, _)) = pinger.ping(tx_addr).await {
                _ = ping_tx.send(NetworkEvent::Pong(msg));
            }
        }
    });

    loop {
        tokio::select! {
            cmd = cm_rx.recv() => match cmd {
                Some(NetworkCommand::Stop) | None => break,
            },
            session = server.accept() => match session {
                Ok(session) => {
                    tasks.spawn(proxy::run(session, tx_addr, capture.clone()));
                }
                Err(err) => {
                    _ = ev_tx.send(NetworkEvent::Error(format!("Listener closed: {err:?}")));
                    break;
                }
            },
        }
    }

    tasks.shutdown().await;
    server.stop().await;

    Ok(())
}

fn spawn_decoder(ev_tx: UnboundedSender<NetworkEvent>) -> UnboundedSender<Captured> {
    let (tx, mut rx) = unbounded_channel::<Captured>();

    std::thread::spawn(move || {
        while let Some(captured) = rx.blocking_recv() {
            let event = match captured {
                Captured::Packet { frame, source, addr } => NetworkEvent::Packet {
                    packet: decode_dyn(&frame),
                    frame,
                    source,
                    addr,
                },
                Captured::Event(event) => event,
            };
            if ev_tx.send(event).is_err() {
                break;
            }
        }
    });

    tx
}
