use crate::{BedrockConnection, BedrockProtocol};
use bedrock::network::codec::{join_batch, split_batch};
use bedrock::protocol::trace::{trace, FieldSpan};
use bedrock::protocol::{PacketDyn, PacketHeader, Packets, ProtoCodec, UnknownPacket};
use std::io::Cursor;

pub type Frame = Vec<u8>;

pub async fn read_frames(conn: &mut BedrockConnection) -> Result<Vec<Frame>, String> {
    let batch = conn.recv_batch().await.map_err(|e| e.to_string())?;
    let frames = split_batch(&batch).map_err(|e| e.to_string())?;
    Ok(frames.into_iter().map(<[u8]>::to_vec).collect())
}

pub async fn write_frames(conn: &mut BedrockConnection, frames: &[Frame]) -> Result<(), String> {
    if frames.is_empty() {
        return Ok(());
    }

    conn.send_batch(join_batch(frames)).await.map_err(|e| e.to_string())
}

pub fn decode(frame: &[u8]) -> Option<BedrockProtocol> {
    let mut cursor = Cursor::new(frame);
    let (packet, _) = BedrockProtocol::deserialize(&mut cursor).ok()?;
    Some(packet)
}

pub fn packet_id(frame: &[u8]) -> Option<u16> {
    PacketHeader::deserialize(&mut Cursor::new(frame)).ok().map(|h| h.packet_id)
}

pub fn decode_dyn(frame: &[u8]) -> Box<dyn PacketDyn> {
    if let Some(packet) = decode(frame) {
        return packet.into();
    }

    let mut cursor = Cursor::new(frame);
    let id = PacketHeader::deserialize(&mut cursor).map(|h| h.packet_id).unwrap_or(u16::MAX);
    let body = &frame[cursor.position() as usize..];

    Box::new(UnknownPacket { id, buf: body.into() })
}

pub fn trace_fields(frame: &[u8]) -> Vec<FieldSpan> {
    let header_len = {
        let mut cursor = Cursor::new(frame);
        PacketHeader::deserialize(&mut cursor).map_or(0, |_| cursor.position() as usize)
    };

    let (_, fields) = trace(frame, BedrockProtocol::deserialize);

    let mut spans = vec![FieldSpan { name: "header", index: None, depth: 0, start: 0, end: header_len }];
    spans.extend(fields);
    spans
}

pub fn encode(packet: &BedrockProtocol) -> Frame {
    let header = PacketHeader {
        packet_id: packet.id(),
        sender_sub_client_id: 0,
        target_sub_client_id: 0,
    };

    let mut buf = Vec::with_capacity(packet.size_hint(&header));
    packet.serialize(&header, &mut buf).expect("packet serialization into a Vec cannot fail");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock::protocol::v662::packets::RequestNetworkSettingsPacket;

    #[test]
    fn trace_fields_maps_header_and_fields() {
        let packet = BedrockProtocol::RequestNetworkSettingsPacket(Box::new(RequestNetworkSettingsPacket {
            client_network_version: 2193,
        }));
        let frame = encode(&packet);

        let spans: Vec<_> = trace_fields(&frame).iter().map(|s| (s.name, s.start, s.end)).collect();

        assert_eq!(spans, vec![("header", 0, 2), ("client_network_version", 2, 6)]);
    }
}
