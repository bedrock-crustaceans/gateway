use crate::network::command::NetworkCommand;
use crate::network::codec::decode_dyn;
use crate::network::event::{Captured, NetworkEvent};
use crate::BedrockProtocol;
use bedrock::network::nethernet::IdentityStore;
use bedrock::network::tokio::transport::NetherNetConnection;
use bedrock::network::tokio::Listener;
use bedrock::protocol::ProtoVersion;
use raknet_tokio::prelude::RakClient;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;
use tokio::time::interval;

const NETHERNET_IDENTITY_PATH: &str = "nethernet.pem";

pub mod codec;
pub mod command;
pub mod event;
pub mod login;
pub mod proxy;
pub mod source;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transport {
    #[default]
    RakNet,
    NetherNet,
}

impl Transport {
    pub const ALL: [Transport; 2] = [Transport::RakNet, Transport::NetherNet];

    pub fn label(self) -> &'static str {
        match self {
            Transport::RakNet => "RakNet",
            Transport::NetherNet => "NetherNet (HTTP)",
        }
    }
}

pub struct Network {
    pub transport: Transport,
    pub rx_addr: SocketAddr,
    pub tx_addr: SocketAddr,

    pub cm_tx: UnboundedSender<NetworkCommand>,
    pub ev_rx: UnboundedReceiver<NetworkEvent>,
}

impl Network {
    pub fn new(transport: Transport, rx_addr: SocketAddr, tx_addr: SocketAddr) -> Self {
        let (cm_tx, cm_rx) = unbounded_channel();
        let (ev_tx, ev_rx) = unbounded_channel();

        tokio::spawn(async move {
            if let Err(err) = run(transport, rx_addr, tx_addr, cm_rx, ev_tx.clone()).await {
                _ = ev_tx.send(NetworkEvent::Error(err));
            }
        });

        Network {
            transport,
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
    transport: Transport,
    rx_addr: SocketAddr,
    tx_addr: SocketAddr,
    mut cm_rx: UnboundedReceiver<NetworkCommand>,
    ev_tx: UnboundedSender<NetworkEvent>,
) -> Result<(), String> {
    let builder = Listener::builder()
        .name("Gateway")
        .sub_name("https://bedrock-crustaceans.org/gateway")
        .max_players(1000)
        .protocol::<BedrockProtocol>();
    let builder = match transport {
        Transport::RakNet => builder.raknet(rx_addr),
        Transport::NetherNet => {
            let store = IdentityStore::load_or_generate(NETHERNET_IDENTITY_PATH).map_err(|e| format!("Failed to prepare NetherNet identity: {e}"))?;
            builder.nethernet_http(rx_addr, store.identity().map_err(|e| format!("Invalid NetherNet identity: {e}"))?)
        }
    };
    let mut server = builder.bind().await.map_err(|e| format!("Failed to bind {rx_addr}: {e}"))?;

    let capture = spawn_decoder(ev_tx.clone());
    let mut tasks = JoinSet::new();

    let ping_tx = ev_tx.clone();
    match transport {
        Transport::RakNet => {
            let mut pinger = RakClient::new(|conf| conf.protocol = BedrockProtocol::RAKNET_VERSION);
            pinger.start().await.map_err(|e| format!("Failed to start client: {e:?}"))?;

            tasks.spawn(async move {
                let mut tick = interval(Duration::from_secs(3));
                loop {
                    tick.tick().await;
                    if let Ok((msg, _)) = pinger.ping(tx_addr).await {
                        _ = ping_tx.send(NetworkEvent::Pong(msg));
                    }
                }
            });
        }
        Transport::NetherNet => {
            tasks.spawn(async move {
                let mut tick = interval(Duration::from_secs(3));
                loop {
                    tick.tick().await;
                    if let Ok(motd) = NetherNetConnection::query_http(tx_addr).await {
                        _ = ping_tx.send(NetworkEvent::Pong(Box::<[u8]>::from(&motd)));
                    }
                }
            });
        }
    }

    loop {
        tokio::select! {
            cmd = cm_rx.recv() => match cmd {
                Some(NetworkCommand::Stop) | None => break,
            },
            connection = server.accept::<BedrockProtocol>() => match connection {
                Ok(connection) => {
                    tasks.spawn(proxy::run(transport, connection, tx_addr, capture.clone()));
                }
                Err(err) => {
                    _ = ev_tx.send(NetworkEvent::Error(format!("Listener closed: {err:?}")));
                    break;
                }
            },
        }
    }

    tasks.shutdown().await;
    _ = server.shutdown().await;

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
