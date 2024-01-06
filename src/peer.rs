use std::fmt::{Debug, Display, Formatter, Write};
use std::net::Ipv4Addr;
use crate::piece::Piece;
struct PeerVisitor;

impl From<&[u8]> for Peer {
    fn from(bytes: &[u8]) -> Self {
        let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);
        let port = u16::from_be_bytes([bytes[4], bytes[5]]);
        Peer { ip, port, pieces: vec![]}
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{}:{}", self.ip, self.port))
    }
}

pub struct Peer {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub pieces: Vec<Piece>,
}