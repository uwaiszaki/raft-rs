use std::fmt;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io::Cursor;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Clone, Copy)]
pub struct NodeId(pub u16);

impl From<u16> for NodeId {
    fn from(value: u16) -> Self {
        NodeId(value)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub cmd: Bytes,
}

impl LogEntry {
    pub fn encode_to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u64(self.index);
        buf.put_u64(self.term);
        buf.put_slice(&self.cmd);
        buf.freeze()
    }

    pub fn decode(buf: &[u8]) -> Self {
        let mut cursor = Cursor::new(buf);
        let index = cursor.get_u64();
        let term = cursor.get_u64();
        let cmd = Bytes::copy_from_slice(&buf[cursor.position() as usize..]);

        Self {
            term,
            index,
            cmd
        }
    }
}