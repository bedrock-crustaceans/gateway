use bedrock::network::compression::Compression;
use crate::BedrockProtocol;

pub struct Session {}

impl Session {
    pub fn send(&mut self, packet: BedrockProtocol) {
        todo!()
    }
    
    pub fn send_immediate(&mut self, packet: BedrockProtocol) {
        todo!()
    }
    
    pub fn set_compression(&mut self, compression: Option<Compression>) {
        todo!()
    }
}