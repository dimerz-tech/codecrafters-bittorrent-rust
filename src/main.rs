extern crate core;

use std::env;
use serde_json;
use serde_bencode;
use serde::{Deserialize, Serialize};
use sha1::{Sha1, Digest};
use hex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream};
use u16;

fn bencode_to_serde(value: serde_bencode::value::Value) -> serde_json::Value {
    match value {
        serde_bencode::value::Value::Bytes(bytes) => {
            serde_json::Value::String(String::from_utf8_lossy(bytes.as_slice()).to_string())
        },
        serde_bencode::value::Value::Int(int) => {
            serde_json::Value::Number(serde_json::value::Number::from(int))
        },
        serde_bencode::value::Value::List(list) => {
            serde_json::Value::Array(list.into_iter().map(|el| bencode_to_serde(el)).collect())
        },
        serde_bencode::value::Value::Dict(dict) => {
            serde_json::Value::Object(dict.into_iter().map(|el|
                (String::from_utf8_lossy(el.0.as_slice()).to_string(), bencode_to_serde(el.1))).collect())
        }
    }
}


#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    let value = serde_bencode::from_str(encoded_value).unwrap();
    bencode_to_serde(value)
}

#[derive(Debug, Deserialize)]
struct MetaInfo {
    announce: String,
    info: Info
}

#[derive(Debug, Deserialize, Serialize)]
struct Info {
    length: usize,
    name: String,
    #[serde(rename = "piece length")]
    piece_length: usize,
    pieces: serde_bytes::ByteBuf
}

#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    pub complete: usize,
    pub incomplete: usize,
    pub interval: usize,
    #[serde(rename = "min interval")]
    pub min_interval: usize,
    #[serde(with = "serde_bytes")]
    pub peers: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct Torrent {
    meta: MetaInfo,
    hash: [u8; 20]
}

impl Torrent {
    pub fn new(file_path: &String) -> Self {
        let buf = std::fs::read(file_path).unwrap();
        let mut hasher = Sha1::new();
        let meta =  serde_bencode::de::from_bytes::<MetaInfo>(&buf).unwrap();
        let bytes = serde_bencode::to_bytes(&meta.info).unwrap();
        hasher.update(bytes);
        let hash: [u8; 20] = hasher.finalize().try_into().unwrap();
        Torrent { meta, hash }
    }
    pub async fn get_peers(&self) -> Vec<String> {
        let peer_id = "00112233445566778899";
        let port = 6881;
        let uploaded = 0;
        let downloaded = 0;
        let left = self.meta.info.length;
        let compact = 1;
        let info_hash :String = hex::encode(self.hash).chars().
            collect::<Vec<char>>().chunks(2).fold(String::new(), |acc, el| acc + "%" + &*el.iter().collect::<String>());
        let url = format!("{}?info_hash={}&peer_id={peer_id}&port={port}&\
        uploaded={uploaded}&downloaded={downloaded}&left={left}&compact={compact}", self.meta.announce, info_hash);
        let res = reqwest::get(url).await.unwrap();
        let resp: Response = serde_bencode::from_bytes(res.bytes().await.unwrap().as_ref()).unwrap();
        let peers: Vec<String> = resp.peers.chunks(6)
            .map(|peer| format!("{}.{}.{}.{}:{}", peer[0].to_string(),
                                peer[1].to_string(),
                                peer[2].to_string(),
                                peer[3].to_string(),
                                u16::from_be_bytes([peer[4].clone(), peer[5].clone()]))).collect();
        peers
    }
}

async fn connect_peer(peer: &str) -> TcpStream {
    let mut stream = TcpStream::connect(peer).await.unwrap();
    stream
}

#[derive(Debug, Deserialize)]
struct HandShake {
    proto_len: [u8; 1],
    bit_torrent_str: [u8; 19],
    zeros: [u8; 8],
    sha1_info_hash: [u8; 20],
    peer_id: [u8; 20],
}

impl HandShake {
    fn new(hash: [u8; 20]) -> Self {
        let proto_len: [u8; 1] = [19];
        let bit_torrent_str: [u8; 19] = "BitTorrent protocol".as_bytes().try_into().unwrap();
        let zeros: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        let sha1_info_hash = hash;
        let peer_id: [u8; 20] = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9];
        HandShake {proto_len, bit_torrent_str, zeros, sha1_info_hash, peer_id}
    }
}

impl From<[u8; 68]> for HandShake {
    fn from(value: [u8; 68]) -> Self {
        let mut hand_shake = HandShake::new([0u8; 20]);
        hand_shake.peer_id.clone_from_slice(&value[48..]);
        hand_shake
    }
}

async fn handshake(stream: &mut TcpStream, hash: [u8; 20]) {
    let client_hello = HandShake::new(hash.clone());
    let hello_req = [client_hello.proto_len.as_slice(),
        client_hello.bit_torrent_str.as_slice(),
        client_hello.zeros.as_slice(),
        client_hello.sha1_info_hash.as_slice(),
        client_hello.peer_id.as_slice()].concat();
    stream.write_all(hello_req.as_slice()).await.unwrap();
    let mut buf = [0u8; 68];
    stream.read_exact(&mut buf).await.unwrap();
    let peer_hello = HandShake::from(buf);
    println!("Peer ID: {}", hex::encode(peer_hello.peer_id));
}

async fn get_bitfield(stream: &mut TcpStream) -> Vec<usize> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).await.unwrap();
    let mut id = 0u8;
    stream.read_exact(std::slice::from_mut(&mut id)).await.unwrap();
    assert_eq!(id, 5u8);
    let mut buf = vec![0u8; (u32::from_be_bytes(len) - 1) as usize];
    stream.read_exact(&mut buf).await.unwrap();
    let s = buf.into_iter().fold("".to_string(), |s, b| s + &format!("{:08b}", b));
    let pos = s.chars().enumerate().filter(|(_, r)| r == &'1').map(|(index, _)| index).collect::<Vec<_>>();
    println!("Bitfield positions: {:?}", pos);
    pos
}

async fn send_interested(stream: &mut TcpStream) {
    let prefix =  [0u8, 0u8, 0u8, 1u8];
    let id = [2u8];
    stream.write_all(&[prefix.as_slice(), id.as_slice()].concat()).await.unwrap();
}

async fn get_unchoke(stream: &mut TcpStream) {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).await.unwrap();
    let mut id = 0u8;
    stream.read_exact(std::slice::from_mut(&mut id)).await.unwrap();
    assert_eq!(id, 1u8);
}

async fn block_request(stream: &mut TcpStream, index: i32, chunk: i32) {
    let begin = (index * chunk).to_be_bytes();
    let length = chunk.to_be_bytes();
    let prefix =  [0u8, 0u8, 0u8, 13u8];
    let position = index.to_be_bytes();
    let id = [6u8];
    let request = [prefix.as_slice(), id.as_slice(), position.as_slice(), begin.as_slice(), length.as_slice()].concat();
    println!("Begin {:?}", begin);
    println!("Length {:?}", length);
    println!("Index {:?}", position);
    stream.write_all(&request).await.unwrap();
}

async fn block_response(stream: &mut TcpStream, index: i32) -> Vec<u8> {
    println!("Waiting for block.... {}", index);
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).await.unwrap();
    let mut id = 0u8;
    stream.read_exact(std::slice::from_mut(&mut id)).await.unwrap();
    assert_eq!(id, 7u8);
    let mut position = [0u8, 0u8, 0u8, 0u8];
    stream.read_exact(&mut position).await.unwrap();
    assert_eq!(i32::from_be_bytes(position), index);
    let mut begin = [0u8, 0u8, 0u8, 0u8];
    stream.read_exact(&mut begin).await.unwrap();
    let mut buf = vec![0u8; (i32::from_be_bytes(len) - 9) as usize];
    stream.read_exact(&mut buf).await.unwrap();
    println!("Block {} is downloaded", index);
    buf
}

async fn load_piece(stream: &mut TcpStream, piece: i32, torrent: &Torrent) {
    let file_size = torrent.meta.info.length.clone() as i32;
    let mut piece_size: i32 = torrent.meta.info.piece_length.clone() as i32;
    let (int_pieces, remainder_piece) = (&file_size / &piece_size, &file_size % &piece_size);
    if piece == int_pieces && remainder_piece != 0 {
        piece_size = remainder_piece
    }
    println!("File size {}, piece size {}", file_size, piece_size);
    let chunk: i32 = 16 * 1024;
    let (int_block, remainder_block) = (&piece_size / &chunk, &piece_size % &chunk);
    let mut loaded_piece: Vec<u8> = Vec::new();
    for i in 0..int_block {
        println!("Downloading block {}, size {}........", i, chunk);
        block_request(stream, i, chunk.clone()).await;
        let mut block = block_response(stream, i.clone()).await;
        loaded_piece.append(&mut block);
    }
    println!("Remainded block");
    if remainder_block != 0 {
        println!("Downloading block {}, size {}........", int_block, remainder_block);
        block_request(stream, int_block, remainder_block).await;
        let mut block = block_response(stream, int_block.clone()).await;
        loaded_piece.append(&mut block);
    }
}


// Usage: your_bittorrent.sh decode "<encoded_value>"
#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];
    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value);
    } else if command == "info" {
        let file_path =  &args[2];
        let torrent = Torrent::new(file_path);
        println!("Tracker URL: {}", torrent.meta.announce);
        println!("Length: {}", torrent.meta.info.length);
        println!("Info Hash: {}", hex::encode(torrent.hash));
        println!("Piece Length: {}", torrent.meta.info.piece_length);
        let chunks: Vec<&[u8]> = torrent.meta.info.pieces.as_ref().chunks(20).collect();
        for chunk in chunks {
            println!("{}", hex::encode(chunk));
        }
    } else if command == "peers" {
        let file_path =  &args[2];
        let torrent = Torrent::new(file_path);
        let peers = torrent.get_peers().await;
        println!("{:?}", peers);
    } else if command == "handshake" {
        let file_path =  &args[2];
        let torrent = Torrent::new(file_path);
        let peer = &args[3];
        let mut connection = connect_peer(peer).await;
        handshake(&mut connection, torrent.hash.clone()).await;
    } else if command == "download_piece" {
        let file_path = &args[4];
        let piece_path = &args[3];
        let piece_num: i32 = (&args[5]).parse().unwrap();
        let torrent = Torrent::new(file_path);
        let peers = torrent.get_peers().await;
        let peer = peers.get(0).unwrap();
        let mut connection = connect_peer(peer).await;
        handshake(&mut connection, torrent.hash.clone()).await;
        get_bitfield(&mut connection).await;
        send_interested(&mut connection).await;
        get_unchoke(&mut connection).await;
        load_piece(&mut connection, piece_num, &torrent).await;
        println!("I am here");
    }
    else {
        println!("unknown command: {}", args[1])
    }
}