//! common — shared protocol definitions, no_std edition.
//! (no std is necessary for embedded systems)
//! The Linux version had:
//!   #[cfg(not(target_os = "linux"))]
//!   compile_error!("Linux only");
//!
//! That's gone. This crate now works on ANY target : Linux, Windows, bare metal.
//! It only uses `core`, which is always available regardless of OS.
//!
//! Everything else is identical to the Linux version. This is one of the big
//! wins of Rust's no_std story: protocol logic is truly portable.


/**  
in practice you almost never see /** */ or /*! */ in real Rust code....

the // style is strongly preferred by convention....
Which is EXACTLY why we're breaking it!  :D

THANK YOU https://docs.rust-embedded.org/book/

*/

#![no_std]  // <- we're going embedded, so no std

// ── Protocol constants ───────────────────────────────────────────────────────

/// Custom EtherType — not registered with IEEE.
pub const CUSTOM_ETHERTYPE: u16 = 0x88B6;

/// Maximum application payload in bytes.
/// Reduced from 1024 to 256 — embedded MCUs often have only 64–256 KB of RAM
/// total, so a 1 KB buffer per frame is extravagant.
pub const MAX_PAYLOAD: usize = 256;

/// Ethernet header length: 6 (dst MAC) + 6 (src MAC) + 2 (EtherType).
pub const ETH_HLEN: usize = 14;

/// Ethernet MAC address length in bytes.
pub const ETH_ALEN: usize = 6;

/// Maximum total frame size: Ethernet header + custom header + max payload.
/// Used to size stack-allocated frame buffers.
/// On Linux we used vec![0u8; 65536]. Here we use [u8; MAX_FRAME_SIZE].
pub const MAX_FRAME_SIZE: usize = ETH_HLEN + CustomHeader::SIZE + MAX_PAYLOAD;

// ── Message types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MsgType {
    Data = 0x01,
    Ack  = 0x02,
    Ping = 0x03,
}

impl MsgType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            // Some() is one of the two variants of the Option enum, 
            // which is Rust's way of representing a value that might or might not exist.
            // we use match to handle it when it does have data.....
            0x01 => Some(Self::Data),
            0x02 => Some(Self::Ack),
            0x03 => Some(Self::Ping),
            // .......and for when it doesn't
            _    => None,
        }
    }
}

// ── CustomHeader ─────────────────────────────────────────────────────────────
// Completely unchanged from the Linux version. Byte-level serialization is
// portable by definition because it doesn't depend on the OS at all.


#[derive(Debug, Clone, Copy)]
pub struct CustomHeader {
    pub version:     u8,
    pub msg_type:    u8,
    pub seq:         u16,
    pub payload_len: u16,
    pub checksum:    u32,
}

impl CustomHeader {
    /// Wire size: 1 + 1 + 2 + 2 + 4 = 10 bytes.
    pub const SIZE: usize = 10;

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            version:     bytes[0],
            msg_type:    bytes[1],
            seq:         u16::from_be_bytes([bytes[2], bytes[3]]),
            payload_len: u16::from_be_bytes([bytes[4], bytes[5]]),
            checksum:    u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let seq = self.seq.to_be_bytes();
        let pl  = self.payload_len.to_be_bytes();
        let cs  = self.checksum.to_be_bytes();
        [
            self.version,
            self.msg_type,
            seq[0], seq[1],
            pl[0],  pl[1],
            cs[0],  cs[1], cs[2], cs[3],
        ]
    }
}

// ── Checksum ─────────────────────────────────────────────────────────────────
// Also completely unchanged, pure arithmetic, no OS involvement.

pub fn compute_checksum(data: &[u8]) -> u32 {
    data.iter().map(|&b| b as u32).sum()
}
