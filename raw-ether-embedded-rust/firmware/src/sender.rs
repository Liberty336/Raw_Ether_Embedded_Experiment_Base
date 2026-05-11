//! sender : build and transmit a raw custom-protocol Ethernet frame.
//!
//! On Linux this was a standalone binary with main(), AF_PACKET setup,
//! command-line args, and process::exit(). Here it's a plain function that
//! takes a driver implementing EthernetMac. No OS, no args, no exit.
//!
//! The frame building logic is identical to sender.c : same byte layout,
//! same header fields, same checksum. Only the implementation
//! changed (trait method instead of sendto syscall).

use common::{
    compute_checksum, CustomHeader, MsgType,
    CUSTOM_ETHERTYPE, ETH_ALEN, ETH_HLEN, MAX_PAYLOAD, MAX_FRAME_SIZE,
};
use crate::eth_driver::{EthernetMac, EthError};

/// Send a DATA frame to `dest_mac` containing `payload`.
///
/// `seq` is the sequence number to use (the Linux sender always used 1;
/// on embedded you'd increment this per call using a counter in your state).
///
/// Returns Ok(bytes_sent) or an EthError.
///
/// Linux equivalent: the entire sender main() after argument parsing.
pub fn send_data(
    eth:      &mut impl EthernetMac,
    dest_mac: &[u8; ETH_ALEN],
    payload:  &[u8],
    seq:      u16,
) -> Result<usize, EthError> {
    // Guard: same check as the Linux sender's "Message too long" block
    if payload.len() > MAX_PAYLOAD {
        // No process::exit() on bare metal, so return an error instead.
        // The caller decides what to do (log, blink an LED, retry with less data).
        return Err(EthError::BufferTooSmall);
    }

    let my_mac   = eth.mac_address();
    let frame_len = ETH_HLEN + CustomHeader::SIZE + payload.len();

    // Fixed-size stack buffer instead of vec![0u8; frame_len].
    // We always allocate MAX_FRAME_SIZE bytes; only frame_len of them matter.
    // This avoids a heap allocator entirely : critical on MCUs without one.
    let mut frame = [0u8; MAX_FRAME_SIZE];

    // ── Ethernet header ───────────────────────────────────────────────────────
    // Identical layout to sender.c. Same byte positions, same fields.
    frame[0..6].copy_from_slice(dest_mac);   // destination MAC
    frame[6..12].copy_from_slice(&my_mac);   // source MAC
    let et = CUSTOM_ETHERTYPE.to_be_bytes();
    frame[12] = et[0];
    frame[13] = et[1];

    // ── Custom header ─────────────────────────────────────────────────────────
    let hdr = CustomHeader {
        version:     1,
        msg_type:    MsgType::Data as u8,
        seq,
        payload_len: payload.len() as u16,
        checksum:    compute_checksum(payload),
    };
    frame[ETH_HLEN..ETH_HLEN + CustomHeader::SIZE].copy_from_slice(&hdr.to_bytes());

    // ── Payload ───────────────────────────────────────────────────────────────
    frame[ETH_HLEN + CustomHeader::SIZE..ETH_HLEN + CustomHeader::SIZE + payload.len()]
        .copy_from_slice(payload);

    // ── Transmit ──────────────────────────────────────────────────────────────
    // Linux: sendto(fd, frame.as_ptr(), frame_len, 0, &saddr, sizeof(saddr))
    // Embedded: eth.send(&frame[..frame_len])
    // The trait implementor does all the hardware-specific work for us.
    eth.send(&frame[..frame_len])
}
