use crate::BedrockConnection;

pub enum NetworkEvent {
    Started,
    Stopped,
    NewConn(BedrockConnection)
}