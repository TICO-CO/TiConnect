use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

use ticonnect_net::transfer::{Chunk, ChunkError};
use ticonnect_net::transport::PeerInfo;

pub struct InboundTransfer {
    pub peer: PeerInfo,
    pub send: quinn::SendStream,
    pub recv: quinn::RecvStream,
    pub frame: ticonnect_net::transfer::TransferFrame,
}

pub fn write_chunk(file: &mut File, chunk: &Chunk) -> Result<(), ChunkError> {
    file.seek(SeekFrom::Start(chunk.offset()))
        .map_err(|error| ChunkError::Io(error.to_string()))?;
    file.write_all(chunk.data())
        .map_err(|error| ChunkError::Io(error.to_string()))?;
    Ok(())
}
