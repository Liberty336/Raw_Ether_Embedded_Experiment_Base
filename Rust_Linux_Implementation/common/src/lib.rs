//! common — shared protocol definitions.
//! This is the Rust equivalent of common.h.
//!
//! In Rust, shared code lives in a *library crate* rather than a header file.
//! Cargo handles compilation and linking — no #include, no header guards needed.

// Hard compile error on non-Linux, same intent as the #warning in the C code
// but stronger. cfg() attributes are evaluated at compile time by rustc.
#[cfg(not(target_os = "linux"))]
compile_error!("This crate only supports Linux (requires AF_PACKET raw sockets)");

// ── Protocol constants ───────────────────────────────────────────────────────
// In C these were #define macros. In Rust, `const` is strongly typed, scoped,
// and usable in pattern matches — strictly better than a macro for constants.

/// Custom EtherType — not registered with IEEE.
pub const CUSTOM_ETHERTYPE: u16 = 0x88B6;

/// Maximum application payload in bytes.
pub const MAX_PAYLOAD: usize = 1024;

/// Ethernet header length: 6 (dst MAC) + 6 (src MAC) + 2 (EtherType).
pub const ETH_HLEN: usize = 14;

/// Ethernet MAC address length in bytes.
pub const ETH_ALEN: usize = 6;

// ── Message types ────────────────────────────────────────────────────────────
// In C these were bare #define constants. In Rust we use an enum.
// The compiler will warn if we forget a variant in a match arm (exhaustiveness
// checking) — something C cannot do with plain integer constants.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]   // tells the compiler: store this enum as a single byte, like uint8_t
pub enum MsgType {
    Data = 0x01,
    Ack  = 0x02,
    Ping = 0x03,
}

impl MsgType {
    /// Convert a raw u8 byte into a MsgType, or None if unrecognised.
    /// Equivalent to a switch/case over the raw msg_type byte in C.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Data),
            0x02 => Some(Self::Ack),
            0x03 => Some(Self::Ping),
            _    => None,
        }
    }
}

// ── CustomHeader ─────────────────────────────────────────────────────────────
// In C this was __attribute__((packed)) struct written into a byte buffer via memcpy.
//
// We deliberately do NOT use #[repr(C, packed)] here.
// Why? Reading fields of a packed struct in Rust requires unsafe{} because the
// fields may be misaligned in memory. It's easy to silently trigger undefined
// behaviour. Instead we use explicit byte-level serialisation (from_bytes /
// to_bytes), which is safe, clear, and handles byte order explicitly.
//
// This struct holds fields in HOST byte order (already converted from network order).
// It is the "decoded" version — safe to use in arithmetic and comparisons.

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

    /// Deserialise from a network-byte-order (big-endian) byte slice.
    /// This is the equivalent of:
    ///   memcpy(&hdr, frame + ETH_HLEN, sizeof(hdr));
    ///   hdr.seq = ntohs(hdr.seq);  etc.
    ///
    /// Returns None if the slice is too short — no undefined behaviour possible.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            version:  bytes[0],
            msg_type: bytes[1],
            // u16::from_be_bytes() is the Rust equivalent of ntohs()
            seq:         u16::from_be_bytes([bytes[2], bytes[3]]),
            payload_len: u16::from_be_bytes([bytes[4], bytes[5]]),
            // u32::from_be_bytes() is the Rust equivalent of ntohl()
            checksum:    u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
        })
    }

    /// Serialise to network-byte-order bytes, ready to copy into a frame.
    /// Equivalent to htons()/htonl() conversions followed by memcpy() in C.
    /// .to_be_bytes() is the Rust equivalent of htons() / htonl().
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

/// Simple additive checksum — identical algorithm to compute_checksum() in C.
///
/// In Rust we take a slice (&[u8]) instead of a raw pointer + length.
/// The slice carries its own length, is bounds-checked, and cannot overread.
/// No unsafe needed. No off-by-one possible.
pub fn compute_checksum(data: &[u8]) -> u32 {
    // .iter() walks each byte reference, .map() widens u8 → u32 to match the
    // C version's uint32_t accumulator, .sum() folds into the total.
    data.iter().map(|&b| b as u32).sum()
}
