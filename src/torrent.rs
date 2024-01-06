use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

#[derive(Debug, Deserialize)]
pub struct Torrent {
    pub meta: MetaInfo,
    pub info_hash: [u8; 20]
}

impl Torrent {
    pub fn new(file_path: &String) -> Self {
        let buf = std::fs::read(file_path).unwrap();
        let mut hasher = Sha1::new();
        let meta =  serde_bencode::de::from_bytes::<MetaInfo>(&buf).unwrap();
        let bytes = serde_bencode::to_bytes(&meta.info).unwrap();
        hasher.update(bytes);
        let hash: [u8; 20] = hasher.finalize().try_into().unwrap();
        Torrent { meta, info_hash: hash }
    }
}

/*
 The content of a metainfo file (the file ending in ".torrent") is a bencoded dictionary,
 containing the keys listed below. All character string values are UTF-8 encoded.
*/
#[derive(Debug, Deserialize)]
pub struct MetaInfo {
    // The announce URL of the tracker (string)
    pub announce: String,
    // a dictionary that describes the file(s) of the torrent.
    pub info: Info
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Info {
    // length of the file in bytes (integer)
    pub length: i32,
    // the filename. This is purely advisory. (string)
    pub name: String,
    // number of bytes in each piece (integer)
    #[serde(rename = "piece length")]
    pub piece_length: i32,
    // string consisting of the concatenation of all 20-byte SHA1 hash values,
    // one per piece (byte string, i.e. not urlencoded)
    pub pieces: serde_bytes::ByteBuf
}
