use crate::{BedrockConnection, BedrockProtocol};
use bedrock::network::error::ListenerError;
use bedrock::network::listener::Listener;
use bedrock::protocol::ProtoVersion;
use std::net::SocketAddr;
use tokio::sync::mpsc::Sender;

pub async fn run_listener(addr: SocketAddr, conn_tx: Sender<BedrockConnection>) -> Result<(), ListenerError> {
    let mut listener = Listener::new_raknet(
        addr, 
        "Gateway".to_string(),
        "https://bedrock-crustaceans.org/gateway".to_string(),
        BedrockProtocol::GAME_VERSION.to_string(),
        BedrockProtocol::PROTOCOL_VERSION,
        BedrockProtocol::RAKNET_VERSION,
        0,
        0,
        false,
    ).await?;
    
    listener.start().await?;
    
    loop {
        let Ok(conn) = listener.accept::<BedrockProtocol>().await else { break };
        
        _ = conn_tx.send(conn).await;
    }
    
    listener.start().await?;
    
    Ok(())
}