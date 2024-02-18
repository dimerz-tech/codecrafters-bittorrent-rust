mod torrent;
mod tracker;
mod peer;
mod piece;

use std::env;
use std::str::FromStr;
use anyhow::anyhow;
use serde_json;
use serde_bencode;
use serde::{Deserialize, Serialize};
use sha1::{Sha1, Digest};
use hex;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream};
use u16;
use crate::peer::Peer;
use crate::torrent::Torrent;

fn bencode_to_serde(value: serde_bencode::value::Value) -> serde_json::Value {
    match value {
        serde_bencode::value::Value::Bytes(bytes) => {
            serde_json::Value::String(String::from_utf8_lossy(bytes.as_slice()).to_string())
        }
        serde_bencode::value::Value::Int(int) => {
            serde_json::Value::Number(serde_json::value::Number::from(int))
        }
        serde_bencode::value::Value::List(list) => {
            serde_json::Value::Array(list.into_iter().map(|el| bencode_to_serde(el)).collect())
        }
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




async fn connect_peer(peer: &str) -> TcpStream {
    let stream = TcpStream::connect(peer).await.unwrap();
    stream
}

async fn send_interested(stream: &mut TcpStream) {
    let prefix = [0u8, 0u8, 0u8, 1u8];
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

async fn block_request(stream: &mut TcpStream, piece: i32, position: i32, block: i32) {
    let index = piece.to_be_bytes();
    let begin = position.to_be_bytes();
    let length = block.to_be_bytes();
    let prefix = [0u8, 0u8, 0u8, 13u8];
    let id = [6u8];
    let request = [prefix.as_slice(), id.as_slice(), index.as_slice(), begin.as_slice(), length.as_slice()].concat();
    println!("Begin {:?}", begin);
    println!("Length {:?}", length);
    println!("Index {:?}", index);

    stream.write_all(&request).await.unwrap();
}

async fn block_response(stream: &mut TcpStream, index: i32) -> Vec<u8> {
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
    buf
}

async fn load_piece(stream: &mut TcpStream, piece: i32, torrent: &Torrent) -> Vec<u8> {
    let file_size = torrent.meta.info.length.clone() as i32;
    let piece_len: i32 = torrent.meta.info.piece_length.clone() as i32;
    let piece_size = piece_len.min(file_size - piece_len * piece);
    println!("File size {}, Piece {}, piece size {}", file_size, piece, piece_size);
    const BLOCK_SIZE: i32 = 16 * 1024;
    let mut loaded_piece: Vec<u8> = Vec::new();
    let mut remaining_block = piece_size;
    while remaining_block > 0 {
        let position = piece_size - remaining_block;
        let block_size = BLOCK_SIZE.min(remaining_block);
        println!("Block position {}, block size {}", position, block_size);
        block_request(stream, piece, position, block_size).await;
        let mut block = block_response(stream, piece).await;
        loaded_piece.append(&mut block);
        println!("File size: {}", loaded_piece.len());
        remaining_block -= block_size;
    }
    let chunks: Vec<&[u8]> = torrent.meta.info.pieces.as_ref().chunks(20).collect();
    let piece_hash = chunks[piece as usize];
    let mut hasher = Sha1::new();
    hasher.update(&loaded_piece);
    let hash: [u8; 20] = hasher.finalize().try_into().unwrap();
    assert_eq!(hash, piece_hash);
    loaded_piece
}

async fn write_file(path: &String, data: &Vec<u8>) {
    let mut file = File::create(path).await.unwrap();
    file.write_all(data).await.unwrap();
}


// Usage: your_bittorrent.sh decode "<encoded_value>"
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];
    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value);
        Ok(())
    } else if command == "info" {
        let file_path = &args[2];
        let torrent = Torrent::new(file_path);
        println!("Tracker URL: {}", torrent.meta.announce);
        println!("Length: {}", torrent.meta.info.length);
        println!("Info Hash: {}", hex::encode(torrent.info_hash));
        println!("Piece Length: {}", torrent.meta.info.piece_length);
        let chunks: Vec<&[u8]> = torrent.meta.info.pieces.as_ref().chunks(20).collect();
        for chunk in chunks {
            println!("{}", hex::encode(chunk));
        }
        Ok(())
    } else if command == "peers" {
        let file_path = &args[2];
        let torrent = Torrent::new(file_path);
        let peers = tracker::get_peers(&torrent).await?;
        for peer in peers {
            println!("{}", peer);
        }
        Ok(())
    } else if command == "handshake" {
        let file_path = &args[2];
        let torrent = Torrent::new(file_path);
        let peer = &args[3];
        let mut peer = Peer::from_str(peer.as_str())?;
        peer.handshake(torrent.info_hash).await?;
        Ok(())
    } else if command == "download_piece" {
        let file_path = &args[4];
        let piece_path = &args[3];
        let piece_num: i32 = (&args[5]).parse().unwrap();
        let torrent = Torrent::new(file_path);
        let peers = tracker::get_peers(&torrent).await?;
        for mut peer in peers {
            peer.handshake(torrent.info_hash).await?;
            peer.load_piece(piece_num as usize).await?;
            // send_interested(&mut connection).await;
            // get_unchoke(&mut connection).await;
            // let loaded_piece = load_piece(&mut connection, piece_num, &torrent).await;
            // write_file(piece_path, &loaded_piece).await;
        }
        println!("I am here");
        Ok(())
    } else if command == "download" {
        // let torrent_path = &args[4];
        // let file_path = &args[3];
        // let torrent = Torrent::new(torrent_path);
        // let peers = torrent.get_peers().await;
        // let peer = peers.get(0).unwrap();
        // let mut connection = connect_peer(peer).await;
        // peer.handshake(&mut connection, torrent.info_hash.clone()).await;
        // peer.get_bitfield(&mut connection).await;
        // send_interested(&mut connection).await;
        // get_unchoke(&mut connection).await;
        // let pieces: Vec<&[u8]> = torrent.meta.info.pieces.as_ref().chunks(20).collect();
        // let mut loaded_pieces: Vec<u8> = Vec::new();
        // for (i, _) in pieces.into_iter().enumerate() {
        //     let mut piece = load_piece(&mut connection, i as i32, &torrent).await;
        //     loaded_pieces.append(&mut piece);
        // }
        // write_file(file_path, &loaded_pieces).await;
        Ok(())
    } else {
        println!("unknown command: {}", args[1]);
        Err(anyhow!("Unknown command"))
    }
}