//! receiver — poll for incoming frames and send ACK replies.
//!
//! The Linux version blocked in recvfrom() waiting for a frame.
//! On bare metal we can't block — there's no OS to context-switch us out.
//! Instead, poll() returns immediately: it either processes one frame or
//! returns Err(EthError::RxEmpty) if nothing is waiting.
//!
//! The main loop in main.rs calls poll() repeatedly. In a real project you'd
//! trigger it from an Ethernet interrupt handler, or use embassy's async/await
//! to yield the CPU while waiting.

use common::{
    compute_checksum, CustomHeader, MsgType,
    CUSTOM_ETHERTYPE, ETH_ALEN, ETH_HLEN, MAX_PAYLOAD, MAX_FRAME_SIZE,
};
use crate::eth_driver::{EthernetMac, EthError};

/// Outcome of one poll() call — what happened to the frame (if any).
/// On Linux we just printed to stdout; here we return structured data
/// because there's no println! on bare metal (no stdout, no OS).
/// The caller can log via RTT, semihosting, a serial UART, etc.
#[derive(Debug)]
pub enum PollResult {
    /// No frame was waiting. Try again later.
    NoFrame,
    /// Frame received and processed. ACK sent if it was a DATA frame.
    Processed { msg_type: u8, seq: u16, payload_len: u16 },
    /// Frame arrived but was malformed or failed checksum.
    Rejected,
}

/// Check for a waiting frame, parse it, and send an ACK if appropriate.
///
/// Linux equivalent: one iteration of the receiver's loop{} block,
/// from recvfrom() through sendto().
pub fn poll(eth: &mut impl EthernetMac) -> PollResult {
    // Fixed stack buffer. On Linux: vec![0u8; 65536].
    // 65536 bytes on the stack of a Cortex-M would overflow immediately.
    // MAX_FRAME_SIZE (= 14 + 10 + 256 = 280 bytes) is safe.
    let mut frame = [0u8; MAX_FRAME_SIZE];

    // Non-blocking receive — returns immediately with RxEmpty if nothing waiting.
    let n = match eth.recv(&mut frame) {
        Ok(n)                        => n,
        Err(EthError::RxEmpty)       => return PollResult::NoFrame,
        Err(_)                       => return PollResult::Rejected,
    };

    // ── Minimum length check ──────────────────────────────────────────────────
    if n < ETH_HLEN + CustomHeader::SIZE {
        return PollResult::Rejected;
    }

    // ── EtherType filter ──────────────────────────────────────────────────────
    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
    if ethertype != CUSTOM_ETHERTYPE {
        return PollResult::NoFrame; // not our protocol, silently ignore
    }

    // ── Source MAC ────────────────────────────────────────────────────────────
    // try_into() converts &[u8] to [u8; 6] — safe because we checked n >= ETH_HLEN.
    let src_mac: [u8; ETH_ALEN] = frame[6..12].try_into().unwrap();

    // ── Parse custom header ───────────────────────────────────────────────────
    let hdr = match CustomHeader::from_bytes(&frame[ETH_HLEN..]) {
        Some(h) => h,
        None    => return PollResult::Rejected,
    };

    // ── Payload length validation ─────────────────────────────────────────────
    if hdr.payload_len as usize > MAX_PAYLOAD {
        return PollResult::Rejected;
    }
    let total_needed = ETH_HLEN + CustomHeader::SIZE + hdr.payload_len as usize;
    if n < total_needed {
        return PollResult::Rejected;
    }

    // ── Checksum verification ─────────────────────────────────────────────────
    let payload_start = ETH_HLEN + CustomHeader::SIZE;
    let payload       = &frame[payload_start..payload_start + hdr.payload_len as usize];
    if compute_checksum(payload) != hdr.checksum {
        return PollResult::Rejected;
    }

    // ── ACK reply (only for DATA frames) ─────────────────────────────────────
    if hdr.msg_type == MsgType::Data as u8 {
        send_ack(eth, &src_mac, hdr.seq);
    }

    PollResult::Processed {
        msg_type:    hdr.msg_type,
        seq:         hdr.seq,
        payload_len: hdr.payload_len,
    }
}

/// Build and send an ACK frame back to `dest_mac`.
/// Extracted as its own function to keep poll() readable.
fn send_ack(eth: &mut impl EthernetMac, dest_mac: &[u8; ETH_ALEN], seq: u16) {
    let my_mac      = eth.mac_address();
    let ack_payload = b"Message received";
    let reply_len   = ETH_HLEN + CustomHeader::SIZE + ack_payload.len();
    let mut reply   = [0u8; MAX_FRAME_SIZE];

    // Ethernet header
    reply[0..6].copy_from_slice(dest_mac);
    reply[6..12].copy_from_slice(&my_mac);
    let et = CUSTOM_ETHERTYPE.to_be_bytes();
    reply[12] = et[0];
    reply[13] = et[1];

    // Custom ACK header
    let hdr = CustomHeader {
        version:     1,
        msg_type:    MsgType::Ack as u8,
        seq,
        payload_len: ack_payload.len() as u16,
        checksum:    compute_checksum(ack_payload),
    };
    reply[ETH_HLEN..ETH_HLEN + CustomHeader::SIZE].copy_from_slice(&hdr.to_bytes());
    reply[ETH_HLEN + CustomHeader::SIZE..ETH_HLEN + CustomHeader::SIZE + ack_payload.len()]
        .copy_from_slice(ack_payload);

    // Ignore send errors here — a real implementation might retry or log.
    let _ = eth.send(&reply[..reply_len]);
}
