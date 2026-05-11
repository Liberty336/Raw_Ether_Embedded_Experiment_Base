//! receiver — listens for custom-protocol Ethernet frames, decodes them,
//! and sends an ACK reply for every MSG_DATA frame received.
//! Equivalent to receiver.c.
//!
//! The interface is read from the INTERFACE environment variable at runtime,
//! defaulting to "lo". This replaces the compile-time -DINTERFACE_NAME flag.
//!
//! Usage: sudo INTERFACE=wlp2s0 ./target/debug/receiver

#[cfg(not(target_os = "linux"))]
compile_error!("receiver only supports Linux");

use std::env;
use std::ffi::CStr;
use std::mem;
use std::process;

use common::{
    compute_checksum, CustomHeader, MsgType,
    CUSTOM_ETHERTYPE, ETH_ALEN, ETH_HLEN, MAX_PAYLOAD,
};

// ── ifreq helper structs (same as sender) ────────────────────────────────────

#[repr(C)]
struct IfReqIndex {
    ifr_name:    [libc::c_char; libc::IFNAMSIZ],
    ifr_ifindex: libc::c_int,
}

#[repr(C)]
struct IfReqHwAddr {
    ifr_name:   [libc::c_char; libc::IFNAMSIZ],
    ifr_hwaddr: libc::sockaddr,
}

fn copy_ifname(dst: &mut [libc::c_char; libc::IFNAMSIZ], name: &str) {
    let bytes = name.as_bytes();
    let len = bytes.len().min(libc::IFNAMSIZ - 1);
    for (i, &b) in bytes[..len].iter().enumerate() {
        dst[i] = b as libc::c_char;
    }
    dst[len] = 0;
}

/// Get the MAC address of `iface` via SIOCGIFHWADDR.
/// Equivalent to get_interface_mac() in receiver.c.
/// Returns None on failure instead of -1 — Rust uses Option for "might not exist".
unsafe fn get_interface_mac(sock_fd: i32, iface: &str) -> Option<[u8; ETH_ALEN]> {
    let mut ifr: IfReqHwAddr = mem::zeroed();
    copy_ifname(&mut ifr.ifr_name, iface);
    if libc::ioctl(sock_fd, libc::SIOCGIFHWADDR, &mut ifr as *mut _) < 0 {
        eprintln!("ioctl SIOCGIFHWADDR failed: {}", std::io::Error::last_os_error());
        return None;
    }
    let mut mac = [0u8; ETH_ALEN];
    for i in 0..ETH_ALEN {
        mac[i] = ifr.ifr_hwaddr.sa_data[i] as u8;
    }
    Some(mac)
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    // Runtime interface selection via environment variable.
    // Equivalent to the compile-time -DINTERFACE_NAME='"wlp2s0"' approach in C,
    // but more flexible — no recompile needed to switch interfaces.
    let iface = env::var("INTERFACE").unwrap_or_else(|_| "lo".to_string());
    println!("Using interface: {}", iface);

    // ── Open socket ───────────────────────────────────────────────────────────
    let sock_fd = unsafe {
        libc::socket(
            libc::AF_PACKET,
            libc::SOCK_RAW,
            CUSTOM_ETHERTYPE.to_be() as i32,
        )
    };
    if sock_fd < 0 {
        eprintln!("socket() failed: {}", std::io::Error::last_os_error());
        process::exit(1);
    }
    println!("File Descriptor: {}", sock_fd);
    println!("Listening for EtherType 0x{:04X} frames...", CUSTOM_ETHERTYPE);

    // ── Bind to interface ─────────────────────────────────────────────────────
    // We store bound_ifindex separately so we can use it in the reply block
    // without depending on getsockname() having succeeded — fixing the subtle
    // bug noted in the C code review.
    let bound_ifindex: i32 = unsafe {
        let mut ifr: IfReqIndex = mem::zeroed();
        copy_ifname(&mut ifr.ifr_name, &iface);
        if libc::ioctl(sock_fd, libc::SIOCGIFINDEX, &mut ifr as *mut _) < 0 {
            eprintln!("ioctl SIOCGIFINDEX failed: {}", std::io::Error::last_os_error());
            libc::close(sock_fd);
            process::exit(1);
        }
        let idx = ifr.ifr_ifindex;

        let mut bind_addr: libc::sockaddr_ll = mem::zeroed();
        bind_addr.sll_family   = libc::AF_PACKET as u16;
        bind_addr.sll_protocol = CUSTOM_ETHERTYPE.to_be();
        bind_addr.sll_ifindex  = idx;

        if libc::bind(
            sock_fd,
            &bind_addr as *const libc::sockaddr_ll as *const libc::sockaddr,
            mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        ) < 0 {
            eprintln!("bind() failed: {}", std::io::Error::last_os_error());
            libc::close(sock_fd);
            process::exit(1);
        }
        idx
    };

    // ── Print socket info (equivalent to getsockname block in receiver.c) ─────
    unsafe {
        let mut sll: libc::sockaddr_ll = mem::zeroed();
        let mut salen = mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t;

        if libc::getsockname(
            sock_fd,
            &mut sll as *mut libc::sockaddr_ll as *mut libc::sockaddr,
            &mut salen,
        ) == 0 {
            let mut ifname_buf = [0i8; libc::IFNAMSIZ];
            let name_str = if sll.sll_ifindex > 0 {
                let ptr = libc::if_indextoname(
                    sll.sll_ifindex as u32,
                    ifname_buf.as_mut_ptr(),
                );
                if ptr.is_null() {
                    "(unknown)".to_string()
                } else {
                    CStr::from_ptr(ifname_buf.as_ptr())
                        .to_string_lossy()
                        .into_owned()
                }
            } else {
                "(none)".to_string()
            };

            println!(
                "  getsockname: ifindex={} name={} protocol=0x{:04X}",
                sll.sll_ifindex,
                name_str,
                u16::from_be(sll.sll_protocol)
            );
        } else {
            eprintln!("getsockname failed: {}", std::io::Error::last_os_error());
        }
    }

    // ── Receive loop ──────────────────────────────────────────────────────────
    // Vec<u8> on the heap rather than a stack array. At 65536 bytes a stack
    // allocation is fine in C, but Rust prefers heap for large buffers to avoid
    // stack overflows in threaded contexts.
    let mut frame = vec![0u8; 65536];

    loop {
        let n = unsafe {
            libc::recvfrom(
                sock_fd,
                frame.as_mut_ptr() as *mut libc::c_void,
                frame.len(),
                0,
                std::ptr::null_mut(), // we don't need the sender address here
                std::ptr::null_mut(),
            )
        };

        if n < 0 {
            eprintln!("recvfrom error: {}", std::io::Error::last_os_error());
            continue;
        }
        let n = n as usize;

        // Discard frames too short to contain our headers
        if n < ETH_HLEN + CustomHeader::SIZE {
            println!("Frame too short, discarding.");
            continue;
        }

        // Check EtherType at bytes [12..14]
        // u16::from_be_bytes is ntohs() — converts from network byte order
        let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
        if ethertype != CUSTOM_ETHERTYPE {
            continue; // not our protocol, ignore silently
        }

        // Source MAC is at bytes [6..12] of the Ethernet header
        // try_into() converts a slice to a fixed-size array — panics if lengths
        // don't match, but we already know n >= ETH_HLEN so this is safe.
        let src_mac: [u8; ETH_ALEN] = frame[6..12].try_into().unwrap();
        println!(
            "Frame from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            src_mac[0], src_mac[1], src_mac[2],
            src_mac[3], src_mac[4], src_mac[5]
        );

        // Parse and decode the custom header (handles ntoh* conversions internally)
        let hdr = match CustomHeader::from_bytes(&frame[ETH_HLEN..]) {
            Some(h) => h,
            None    => { println!("Failed to parse header"); continue; }
        };

        println!(
            "  version={}  msg_type=0x{:02X}  seq={}  payload_len={}",
            hdr.version, hdr.msg_type, hdr.seq, hdr.payload_len
        );

        // Validate payload length — same two-part check as the C receiver
        if hdr.payload_len as usize > MAX_PAYLOAD {
            println!("FAIL: payload_len > MAX_PAYLOAD ({})", hdr.payload_len);
            continue;
        }
        let total_needed = ETH_HLEN + CustomHeader::SIZE + hdr.payload_len as usize;
        if n < total_needed {
            println!("FAIL: frame too small (n={}, needed={})", n, total_needed);
            continue;
        }

        // Slice the payload out of the frame buffer — no pointer arithmetic,
        // no manual length tracking, bounds checked automatically by Rust.
        let payload_start = ETH_HLEN + CustomHeader::SIZE;
        let payload       = &frame[payload_start..payload_start + hdr.payload_len as usize];

        // Verify checksum
        let expected = compute_checksum(payload);
        if expected != hdr.checksum {
            println!(
                "  Checksum MISMATCH (got {}, expected {})",
                hdr.checksum, expected
            );
            continue;
        }

        // Print payload. from_utf8_lossy() replaces invalid UTF-8 bytes with
        // the replacement character rather than panicking — equivalent to the
        // %.*s printf format in C (print exactly payload_len bytes regardless
        // of content).
        println!("  payload: \"{}\"", String::from_utf8_lossy(payload));

        // ── Reply block ───────────────────────────────────────────────────────
        // Only reply to DATA frames to avoid infinite ACK loops.
        if hdr.msg_type != MsgType::Data as u8 {
            continue;
        }

        let my_mac = unsafe { get_interface_mac(sock_fd, &iface) };
        let my_mac = match my_mac {
            Some(m) => m,
            None    => { eprintln!("Could not get local MAC, skipping reply"); continue; }
        };

        let ack_payload = b"Message received";
        let reply_len   = ETH_HLEN + CustomHeader::SIZE + ack_payload.len();
        let mut reply   = vec![0u8; reply_len];

        // Ethernet header — swap src and dst compared to what we received
        reply[0..6].copy_from_slice(&src_mac);  // dst = original sender
        reply[6..12].copy_from_slice(&my_mac);  // src = us
        let et = CUSTOM_ETHERTYPE.to_be_bytes();
        reply[12] = et[0];
        reply[13] = et[1];

        // Custom ACK header
        let reply_hdr = CustomHeader {
            version:     1,
            msg_type:    MsgType::Ack as u8,
            seq:         hdr.seq,                          // echo original sequence number
            payload_len: ack_payload.len() as u16,
            checksum:    compute_checksum(ack_payload),
        };
        reply[ETH_HLEN..ETH_HLEN + CustomHeader::SIZE].copy_from_slice(&reply_hdr.to_bytes());
        reply[ETH_HLEN + CustomHeader::SIZE..].copy_from_slice(ack_payload);

        let sent = unsafe {
            let mut reply_addr: libc::sockaddr_ll = mem::zeroed();
            reply_addr.sll_family   = libc::AF_PACKET as u16;
            reply_addr.sll_protocol = CUSTOM_ETHERTYPE.to_be();
            reply_addr.sll_ifindex  = bound_ifindex; // use stored index, not getsockname result
            reply_addr.sll_halen    = ETH_ALEN as u8;
            reply_addr.sll_addr[..ETH_ALEN].copy_from_slice(&src_mac);

            libc::sendto(
                sock_fd,
                reply.as_ptr() as *const libc::c_void,
                reply_len,
                0,
                &reply_addr as *const libc::sockaddr_ll as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };

        if sent < 0 {
            eprintln!("sendto (reply) failed: {}", std::io::Error::last_os_error());
        } else {
            println!(
                "  ACK sent ({} bytes) -> {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                sent,
                src_mac[0], src_mac[1], src_mac[2],
                src_mac[3], src_mac[4], src_mac[5]
            );
        }
    }

    // loop {} never returns, so close() is unreachable — but if you ever
    // add a break condition, add:  unsafe { libc::close(sock_fd); }
}
