use std::fmt::{Debug, Display, Formatter};
use std::net::{Ipv4Addr};
use std::str::FromStr;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use crate::piece::Piece;

impl From<&[u8]> for Peer {
    fn from(bytes: &[u8]) -> Self {
        let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);
        let port = u16::from_be_bytes([bytes[4], bytes[5]]);
        Peer { ip, port, id: [0u8; 20], pieces: vec![], session: None}
    }
}

impl FromStr for Peer {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ip, port) = s.split_once(":").unwrap();
        let ip = Ipv4Addr::from_str(ip)?;
        let port: u16 = port.parse().unwrap();
        Ok(Peer { ip, port, id: [0u8; 20], pieces: vec![], session: None })
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
    pub id: [u8; 20],
    pub pieces: Vec<Piece>,
    pub session: Option<TcpStream>
}

impl Peer {
    pub async fn handshake(&mut self, info_hash: [u8; 20]) -> anyhow::Result<()> {
        let mut stream = TcpStream::connect(format!("{}:{}", self.ip, self.port)).await?;
        stream.write_all(HandShake::new(info_hash).as_bytes_mut()).await?;
        let mut buf = [0u8; 68];
        stream.read_exact(&mut buf).await?;
        let peer_hello = HandShake::from(buf);
        println!("Peer ID: {}", hex::encode(peer_hello.peer_id));
        self.session = Some(stream);
        self.id = peer_hello.peer_id;
        Ok(())
    }
}

// The handshake is a required message and must be the first message transmitted by the client.
#[derive(Debug, Deserialize)]
pub struct HandShake {
    // string length of <pstr>, as a single raw byte
    pstrlen: [u8; 1],
    // string identifier of the protocol
    pstr: [u8; 19],
    // eight (8) reserved bytes. All current implementations use all zeroes
    reserved: [u8; 8],
    // 20-byte SHA1 hash of the info key in the metainfo file
    info_hash: [u8; 20],
    // 20-byte string used as a unique ID for the client.
    peer_id: [u8; 20],
}

impl HandShake {
    fn new(hash: [u8; 20]) -> Self {
        let proto_len: [u8; 1] = [19];
        let bit_torrent_str: [u8; 19] = "BitTorrent protocol".as_bytes().try_into().unwrap();
        let zeros: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        let sha1_info_hash = hash;
        let peer_id: [u8; 20] = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9];
        HandShake { pstrlen: proto_len, pstr: bit_torrent_str, reserved: zeros, info_hash: sha1_info_hash, peer_id }
    }

    // https://github.com/jonhoo implementation struct to byte array
    fn as_bytes_mut(&mut self) -> &mut [u8] {
        let bytes = self as *mut Self as *mut [u8; std::mem::size_of::<Self>()];
        let bytes: &mut [u8; std::mem::size_of::<Self>()] = unsafe { &mut *bytes };
        bytes
    }
}

/*
Create the same structure as for request, but with received peer_id
 */
impl From<[u8; 68]> for HandShake {
    fn from(value: [u8; 68]) -> Self {
        let mut hand_shake = HandShake::new([0u8; 20]);
        hand_shake.peer_id.clone_from_slice(&value[48..]); // get peer id (last 20 bytes) from response
        hand_shake
    }
}

// Communication between peers
struct PeerMessage {
    // The length prefix is a four byte big-endian value
    pub prefix: i32,
    // The message ID is a single decimal byte
    pub id: i32,
    // The payload is message dependent.
    pub payload: Vec<u8>
}
