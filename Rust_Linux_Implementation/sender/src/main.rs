//! sender — builds and transmits a raw custom-protocol Ethernet frame.
//! Equivalent to sender.c.
//!
//! Usage: sudo INTERFACE=wlp2s0 ./target/debug/sender <iface> <dest_mac> <message>
//! e.g.:  sudo ./target/debug/sender wlp2s0 AC:B5:7D:06:40:49 "hello world"

#[cfg(not(target_os = "linux"))]
compile_error!("sender only supports Linux");

use std::env;
use std::mem;
use std::process;

// `common` is our library crate — the Cargo.toml dependency handles linking.
// This replaces #include "common.h"
use common::{
    compute_checksum, CustomHeader, MsgType,
    CUSTOM_ETHERTYPE, ETH_ALEN, ETH_HLEN, MAX_PAYLOAD,
};

// ── ifreq helper structs ─────────────────────────────────────────────────────
// In C, struct ifreq uses a union for different ioctl requests.
// Rust's union syntax is awkward and requires unsafe to access fields.
// Instead we define one struct per ioctl — same #[repr(C)] memory layout,
// no union headaches.

/// ifreq layout for SIOCGIFINDEX — queries the interface's kernel integer index.
#[repr(C)]
struct IfReqIndex {
    ifr_name:    [libc::c_char; libc::IFNAMSIZ],
    ifr_ifindex: libc::c_int,
}

/// ifreq layout for SIOCGIFHWADDR — queries the interface's MAC address.
#[repr(C)]
struct IfReqHwAddr {
    ifr_name:   [libc::c_char; libc::IFNAMSIZ],
    // sockaddr.sa_data[0..6] holds the MAC bytes after the ioctl call
    ifr_hwaddr: libc::sockaddr,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Copy an interface name into a C-style fixed char array.
/// Equivalent to: strncpy(ifr.ifr_name, iface, IFNAMSIZ - 1)
/// We manually null-terminate — same reason as the C code.
fn copy_ifname(dst: &mut [libc::c_char; libc::IFNAMSIZ], name: &str) {
    let bytes = name.as_bytes();
    let len = bytes.len().min(libc::IFNAMSIZ - 1);
    for (i, &b) in bytes[..len].iter().enumerate() {
        dst[i] = b as libc::c_char;
    }
    dst[len] = 0; // null terminator
}

/// Parse "AA:BB:CC:DD:EE:FF" into a [u8; 6].
/// Equivalent to sscanf(mac_str, "%hhx:%hhx:...", &mac[0], ...) in C.
fn parse_mac(s: &str) -> Option<[u8; ETH_ALEN]> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != ETH_ALEN {
        return None;
    }
    let mut mac = [0u8; ETH_ALEN];
    for (i, part) in parts.iter().enumerate() {
        // u8::from_str_radix(part, 16) is equivalent to %hhx in sscanf
        mac[i] = u8::from_str_radix(part, 16).ok()?;
    }
    Some(mac)
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <interface> <dest_mac> <message>", args[0]);
        eprintln!("  e.g: {} wlp2s0 AA:BB:CC:DD:EE:FF \"hello\"", args[0]);
        process::exit(1);
    }

    let iface   = &args[1];
    let mac_str = &args[2];
    let message = args[3].as_bytes(); // &[u8] — no null terminator, just like strlen() in C

    if message.len() > MAX_PAYLOAD {
        eprintln!("Message too long (max {} bytes)", MAX_PAYLOAD);
        process::exit(1);
    }

    let dest_mac = parse_mac(mac_str).unwrap_or_else(|| {
        eprintln!("Invalid MAC address: {}", mac_str);
        process::exit(1);
    });

    // ── Open raw socket ───────────────────────────────────────────────────────
    // From here on we use `unsafe` blocks because we are calling libc functions
    // directly. Rust cannot verify the safety of raw file descriptor operations.
    // This is the same model as C — the difference is Rust forces us to mark
    // these regions explicitly, so it's obvious where unverified code lives.

    let fd = unsafe {
        libc::socket(
            libc::AF_PACKET,
            libc::SOCK_RAW,
            // .to_be() is htons() — converts to network (big-endian) byte order
            (CUSTOM_ETHERTYPE).to_be() as i32,
        )
    };
    if fd < 0 {
        // std::io::Error::last_os_error() reads errno, same as perror() in C
        eprintln!("socket() failed: {}", std::io::Error::last_os_error());
        process::exit(1);
    }
    println!("File Descriptor: {}", fd);

    // ── Get interface index via SIOCGIFINDEX ──────────────────────────────────
    let ifindex = unsafe {
        let mut ifr: IfReqIndex = mem::zeroed(); // equivalent to memset(&ifr, 0, sizeof(ifr))
        copy_ifname(&mut ifr.ifr_name, iface);
        if libc::ioctl(fd, libc::SIOCGIFINDEX, &mut ifr as *mut _) < 0 {
            eprintln!("ioctl SIOCGIFINDEX failed: {}", std::io::Error::last_os_error());
            libc::close(fd);
            process::exit(1);
        }
        ifr.ifr_ifindex
    };

    // ── Get source MAC via SIOCGIFHWADDR ──────────────────────────────────────
    let src_mac: [u8; ETH_ALEN] = unsafe {
        let mut ifr: IfReqHwAddr = mem::zeroed();
        copy_ifname(&mut ifr.ifr_name, iface);
        if libc::ioctl(fd, libc::SIOCGIFHWADDR, &mut ifr as *mut _) < 0 {
            eprintln!("ioctl SIOCGIFHWADDR failed: {}", std::io::Error::last_os_error());
            libc::close(fd);
            process::exit(1);
        }
        // sa_data is [c_char; 14]; the first 6 bytes are the MAC address.
        // Cast c_char → u8 to avoid sign-extension (same reason as the C code's
        // (unsigned char) cast when printing MAC bytes).
        let mut mac = [0u8; ETH_ALEN];
        for i in 0..ETH_ALEN {
            mac[i] = ifr.ifr_hwaddr.sa_data[i] as u8;
        }
        mac
    };

    // ── Build the Ethernet frame ───────────────────────────────────────────────
    // Frame layout (identical to C sender):
    //   [0  .. 6 ) = destination MAC     (6 bytes)
    //   [6  .. 12) = source MAC           (6 bytes)
    //   [12 .. 14) = EtherType            (2 bytes, big-endian)
    //   [14 .. 24) = CustomHeader         (10 bytes)
    //   [24 .. 24+msg_len) = payload

    let msg_len   = message.len();
    let frame_len = ETH_HLEN + CustomHeader::SIZE + msg_len;

    // Vec<u8> on the heap — equivalent to the stack array in C.
    // We use a Vec here so frame_len doesn't need to be a compile-time constant.
    let mut frame = vec![0u8; frame_len];

    // Ethernet header
    frame[0..6].copy_from_slice(&dest_mac);  // destination MAC
    frame[6..12].copy_from_slice(&src_mac);  // source MAC
    // .to_be_bytes() writes the EtherType in big-endian (network) byte order,
    // equivalent to the manual >> 8 / & 0xFF shifts in the C sender.
    let et = CUSTOM_ETHERTYPE.to_be_bytes();
    frame[12] = et[0];
    frame[13] = et[1];

    // Custom header
    let hdr = CustomHeader {
        version:     1,
        msg_type:    MsgType::Data as u8,
        seq:         1,
        payload_len: msg_len as u16,
        checksum:    compute_checksum(message), // checksum over the raw payload bytes
    };
    // to_bytes() handles the htons/htonl conversions internally
    frame[ETH_HLEN..ETH_HLEN + CustomHeader::SIZE].copy_from_slice(&hdr.to_bytes());

    // Payload
    frame[ETH_HLEN + CustomHeader::SIZE..].copy_from_slice(message);

    // ── Send the frame ────────────────────────────────────────────────────────
    let sent = unsafe {
        // sockaddr_ll tells sendto() which interface and destination to use.
        // Equivalent to the sockaddr_ll setup and sendto() call in sender.c.
        let mut saddr: libc::sockaddr_ll = mem::zeroed();
        saddr.sll_family   = libc::AF_PACKET as u16;
        saddr.sll_protocol = CUSTOM_ETHERTYPE.to_be();
        saddr.sll_ifindex  = ifindex;
        saddr.sll_halen    = ETH_ALEN as u8;
        saddr.sll_addr[..ETH_ALEN].copy_from_slice(&dest_mac);

        libc::sendto(
            fd,
            frame.as_ptr() as *const libc::c_void,
            frame_len,
            0,
            &saddr as *const libc::sockaddr_ll as *const libc::sockaddr,
            mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        )
    };

    if sent < 0 {
        eprintln!("sendto() failed: {}", std::io::Error::last_os_error());
        unsafe { libc::close(fd); }
        process::exit(1);
    }

    println!(
        "Sent {} bytes | seq=1 | msg_type=DATA | payload=\"{}\"",
        sent,
        args[3]
    );

    unsafe { libc::close(fd); }
}
