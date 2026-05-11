

/* gcc -std=gnu17 -Wall -Wextra -g -Og -fno-omit-frame-pointer -fstack-protector-strong \
     -D_FORTIFY_SOURCE=2 -fsanitize=address,leak,undefined -pthread sender.c -o sender
*/

// __GNUC__ is a pre-defined macro that GCC knows about.
// So if __GNUC__ is not defined, then they're not compiling with GCC.
// We'll give them an error.
#ifndef __GNUC__
#error "This program must be compiled with GCC"
#endif


#if !defined(__linux__) && !defined(__linux) && !defined(__gnu_linux__)
#warning "This program isn't being compiled on GNU/Linux, program may not work or have undefined behavior"
#endif



// The #include order matters!
// <sys/types.h> is included first so its size_t definitions take precedence over <stdlib.h>'s definitions.



/*
<sys/types.h> is a POSIX header — it's specifically designed to provide the system-level
typedefs our code relies on (pid_t, uid_t, ssize_t, off_t, etc.)

We DO NOT USE <stddef.h> for our size_t implementation, we use <sys/types.h> because it's
specifically defined for POSIX (Linux).

Remember:
<stddef.h> is a C standard header — it's about the language itself, not the OS
<sys/types.h> will give us size_t implementations best for our OS

Key typedefs it provides:
  off_t, ino_t, dev_t, mode_t, nlink_t  — filesystem types
  pid_t, uid_t, gid_t                   — process/user types
  size_t, ssize_t                        — size types (ssize_t can be -1 for errors)
  time_t, clock_t                        — time types
  sa_family_t, socklen_t                 — socket types
*/
#include <sys/types.h>




/*
Including <stdint.h> provides fixed-width integer types.
We NEED these so that the sizes are the exact same even if compiled on different devices.

  uint8_t  is guaranteed to be exactly 8 bits.
  uint16_t is guaranteed to be exactly 16 bits.
  uint32_t is guaranteed to be exactly 32 bits.
*/
#include <stdint.h>




/*
Including <arpa/inet.h> provides declarations for functions and macros that convert Internet
(IPv4/IPv6) addresses between text and binary forms, and handle network/host byte-order
conversions.

  ntohl, ntohs, htonl, htons — convert 32-bit/16-bit values between host and network byte order.

NOTE:
Network byte order is big-endian, so if your system is little-endian, you'll have to convert
before sending over the network. Use hton* when sending, ntoh* when receiving.
*/
#include <arpa/inet.h>




/*
<sys/socket.h> defines socklen_t, the socket() function, AF_PACKET, and many other things.
Key contents:

  Socket functions: socket, bind, send, recv, sendto, recvfrom, shutdown, getsockname
  Address families: AF_INET, AF_INET6, AF_UNIX
  Socket types: SOCK_STREAM, SOCK_DGRAM, SOCK_RAW
  Structs: sockaddr, sockaddr_storage, msghdr, iovec
  Constants: SOMAXCONN, SHUT_RD/SHUT_WR/SHUT_RDWR, MSG_* flags

Not part of standard C; available on POSIX/Unix-like systems.
On Windows, use Winsock (winsock2.h) instead.
*/
#include <sys/socket.h>




/*
<sys/ioctl.h> declares the ioctl() system call and the constants used as its 'request' argument.

ioctl() is a catch-all system call for device and interface operations that don't fit neatly
into the standard read()/write() model. Its signature is:

  int ioctl(int fd, unsigned long request, ...);

  fd      — a file descriptor (socket, device file, etc.)
  request — a code telling the kernel what operation to perform
  arg     — typically a pointer to a struct for passing data in or out

We use it here with socket file descriptors and SIOCGIF* request codes to query the kernel
for network interface properties (index number, MAC address) that aren't exposed through
standard socket APIs.
*/
#include <sys/ioctl.h>




/* <net/if.h> provides:
     IF_NAMESIZE — the maximum interface name buffer size (including null terminator)
     IFNAMSIZ    — same value, the traditional POSIX alias used in struct ifreq
     if_indextoname() — maps an interface index to its name (e.g. "eth0")
     if_nametoindex() — maps an interface name to its kernel index number
     struct ifreq    — the request structure passed to ioctl() for interface queries

Without it, the compiler has no idea any of these exist.
*/
#include <net/if.h>




/* <net/ethernet.h> is needed for raw socket frames, lower level than typical socket programming.

Key definitions:

  struct ether_header {
      u_int8_t  ether_dhost[ETH_ALEN];  // destination MAC
      u_int8_t  ether_shost[ETH_ALEN];  // source MAC
      u_int16_t ether_type;             // protocol type
  };

  ETH_ALEN      = 6      — MAC address length in bytes
  ETH_HLEN      = 14     — Ethernet header length in bytes (6+6+2)
  ETH_DATA_LEN  = 1500   — maximum payload size
  ETH_FRAME_LEN = 1514   — maximum full frame size

  EtherType constants:
  ETHERTYPE_IP   = 0x0800  — IPv4
  ETHERTYPE_ARP  = 0x0806  — ARP
  ETHERTYPE_IPV6 = 0x86DD  — IPv6
*/
#include <net/ethernet.h>




/*
<linux/if_packet.h> pulls in Linux-specific declarations used for AF_PACKET (link-layer / raw
packet) sockets. Key things it provides:

  struct sockaddr_ll — socket address for AF_PACKET, containing:
      sll_family   — address family (AF_PACKET)
      sll_protocol — EtherType in network byte order
      sll_ifindex  — interface index
      sll_halen    — hardware address length
      sll_addr     — hardware (MAC) address

  PACKET_* constants: PACKET_HOST, PACKET_BROADCAST, PACKET_MULTICAST, PACKET_OTHERHOST

This header is Linux-only (not portable).
AF_PACKET sockets let you send/receive raw Ethernet frames (requires CAP_NET_RAW or root).
*/
#include <linux/if_packet.h>




/*
Including <unistd.h> exposes POSIX API declarations for low-level UNIX-like system calls and
constants. We use it for:

  close() — release a file descriptor back to the OS

Constants:
  STDIN_FILENO  — 0
  STDOUT_FILENO — 1
  STDERR_FILENO — 2
*/
#include <unistd.h>




// <stdlib.h> gives us exit(), EXIT_SUCCESS, and EXIT_FAILURE.
#include <stdlib.h>

// <stdio.h> is for printf(), fprintf(), and perror().
#include <stdio.h>

// <string.h> gives us memcpy(), memset(), strlen(), and strncpy().
// We need memset() to zero-initialise structs before filling them in,
// and memcpy() to write fields into the raw frame buffer at exact byte offsets.
#include <string.h>




// THIS MUST BE INCLUDED
// common.h defines CUSTOM_ETHERTYPE, MAX_PAYLOAD, MSG_DATA, and the CustomHeader struct —
// the shared protocol contract between sender and receiver.
// If you add this to your compiler's header directory (like /usr/include), you can use
// #include <common.h> instead.
#include "common.h"




// compute_checksum sums all bytes in a data buffer and returns the total as a 32-bit value.
// This simple additive checksum lets the receiver verify the payload arrived intact —
// if even one byte changed in transit, the sum will differ.
// We use uint32_t because we want our checksum to be EXACTLY 32 bits.
uint32_t compute_checksum(const uint8_t *data, size_t len) {
    uint32_t sum = 0;
    // Iterate over every byte in the payload, accumulating into sum.
    // Using size_t for i matches the type of len, avoiding signed/unsigned comparison warnings.
    for (size_t i = 0; i < len; i++) sum += data[i];
    return sum;
}




// main takes argc/argv so the user can pass the interface name, destination MAC, and message
// as command-line arguments, making the sender flexible without recompiling.
int main(int argc, char *argv[]) {

    // We need exactly three arguments: interface, destination MAC, and message.
    // argc includes the program name itself, so argc < 4 means at least one argument is missing.
    if (argc < 4) {
        // Print usage instructions to stderr so they appear even if stdout is redirected.
        // argv[0] is the program name, so the usage line always shows the actual executable name.
        fprintf(stderr, "Usage: %s <interface> <dest_mac> <message>\n", argv[0]);
        fprintf(stderr, "  e.g: %s eth0 AA:BB:CC:DD:EE:FF \"hello\"\n", argv[0]);
        exit(1);
    }

    // Assign each command-line argument to a named pointer for readability.
    // const char * because we will never modify the argv strings, the OS owns that memory for safety.
    const char *iface        = argv[1];
    const char *dest_mac_str = argv[2];
    const char *message      = argv[3];

    // Parse destination MAC
    // sscanf with %hhx parses a single hex byte (0x00–0xFF) into an unsigned char.
    // We need six of them to fill a 6-byte MAC address array.
    // The return value of sscanf is the number of fields successfully parsed;
    // if it's not exactly 6, the string wasn't a valid MAC address and we bail out.
    uint8_t dest_mac[6];
    if (sscanf(dest_mac_str, "%hhx:%hhx:%hhx:%hhx:%hhx:%hhx",
               &dest_mac[0], &dest_mac[1], &dest_mac[2],
               &dest_mac[3], &dest_mac[4], &dest_mac[5]) != 6) {
        fprintf(stderr, "Invalid MAC address\n");
        exit(1);
    }

    // Open raw socket
    // AF_PACKET → work at the link layer (raw Ethernet frames, not IP/TCP)
    // SOCK_RAW  → we construct and send the full frame ourselves, including the Ethernet header
    // htons(CUSTOM_ETHERTYPE) → tag outgoing frames with our custom EtherType in network byte order,
    //   so only receivers listening for that EtherType will accept them
    int fd = socket(AF_PACKET, SOCK_RAW, htons(CUSTOM_ETHERTYPE));
    if (fd < 0) { perror("socket"); exit(1); }

    // Get interface index and source MAC
    // struct ifreq is the standard request structure for ioctl() interface queries.
    // We fill in the interface name, then pass the struct to ioctl() which fills in the result.
    struct ifreq ifr;

    // Zero the struct before use — uninitialized padding bytes can cause ioctl() to return
    // unexpected results or the kernel to reject the request.
    memset(&ifr, 0, sizeof(ifr));

    // Copy the interface name into ifr_name, leaving the last byte as '\0'.
    // IFNAMSIZ is the maximum interface name buffer size (including null terminator),
    // so "IFNAMSIZ - 1" ensures we never overwrite the terminator.
    strncpy(ifr.ifr_name, iface, IFNAMSIZ - 1);

    // ioctl stands for Input/Output Control. 
    // It's a system call that acts as a catch-all for device and interface operations 
    // that don't fit neatly into the standard read()/write() model.
    
    // Its signature is:
    // int ioctl(int fd, unsigned long request, ...);
    
    // fd — a file descriptor (socket, device file, etc.)
    // request — a code that tells the kernel what you want to do
    // the third argument — usually a pointer to a struct for passing data in or out

    // get the interface's index number
    // SIOCGIFINDEX asks the kernel to fill ifr.ifr_ifindex with the integer index for iface.
    // We need this index to fill in sockaddr_ll.sll_ifindex when calling sendto() later.
    if (ioctl(fd, SIOCGIFINDEX, &ifr) < 0) { perror("ioctl SIOCGIFINDEX"); exit(1); }

    // Save the interface index into its own variable now, because the next ioctl() call
    // will overwrite the same ifr union with hardware address data.
    int ifindex = ifr.ifr_ifindex;

    // get the interface's MAC address
    // SIOCGIFHWADDR asks the kernel to fill ifr.ifr_hwaddr with the interface's hardware address.
    // ifr_hwaddr is a struct sockaddr; its sa_data field holds the raw MAC bytes.
    if (ioctl(fd, SIOCGIFHWADDR, &ifr) < 0) { perror("ioctl SIOCGIFHWADDR"); exit(1); }

    // Cast sa_data to uint8_t* so we can do byte-wise indexing of the 6-byte MAC address.
    // ifr_hwaddr.sa_data is a char array — the cast to uint8_t* avoids signed-char issues
    // when treating individual bytes as unsigned 8-bit values.
    uint8_t *src_mac = (uint8_t *)ifr.ifr_hwaddr.sa_data;

    // Build the frame: [Ethernet header][CustomHeader][payload]
    // Allocate a buffer large enough for the worst-case frame: 14-byte Ethernet header +
    // our fixed-size custom header + the maximum allowed payload.
    // Using a fixed-size stack array is safe here because MAX_PAYLOAD is a compile-time constant.
    uint8_t frame[14 + sizeof(CustomHeader) + MAX_PAYLOAD];

    // Zero the entire frame buffer before writing fields into it.
    // This ensures any bytes we don't explicitly set (e.g. padding at the end) are 0x00
    // rather than whatever garbage was on the stack, preventing accidental data leaks.
    memset(frame, 0, sizeof(frame));

    // Ethernet header
    // The Ethernet header layout is exactly 14 bytes:
    //   bytes  0–5 : destination MAC
    //   bytes  6–11: source MAC
    //   bytes 12–13: EtherType (big-endian)
    memcpy(frame + 0, dest_mac, 6);             // destination MAC
    memcpy(frame + 6, src_mac,  6);             // source MAC

    // Write the EtherType as two individual bytes in big-endian (network) order.
    // >> 8 shifts the high byte down into the low 8 bits so we can store it at offset 12.
    // & 0xFF masks off everything above the lowest 8 bits, giving us just the low byte at offset 13.
    // We do this manually because the frame buffer has no guaranteed alignment for a uint16_t write.
    frame[12] = (CUSTOM_ETHERTYPE >> 8) & 0xFF; // EtherType high byte
    frame[13] = (CUSTOM_ETHERTYPE)      & 0xFF; // EtherType low byte

    // Custom protocol header
    // strlen() returns the number of characters in the message, NOT including the null terminator.
    // We do NOT want to send the null terminator over the wire — the receiver uses payload_len
    // to know where the data ends, so a terminator would just be a wasted byte.
    size_t msg_len = strlen(message);

    // Declare our custom header struct and populate its fields.
    // All multi-byte fields must be converted to network byte order (big-endian) with htons/htonl
    // before we copy the struct into the frame, so the receiver sees the bytes in the right order
    // regardless of the sender's host endianness.
    CustomHeader hdr;
    hdr.version     = 1;                // Protocol version 1 — single byte, no byte-order conversion needed.
    hdr.msg_type    = MSG_DATA;         // Signal to the receiver that this frame carries a data payload.
    hdr.seq         = htons(1);         // Sequence number 1 in network byte order (16-bit, big-endian).
    hdr.payload_len = htons((uint16_t)msg_len);                          // Payload length in network byte order.
    hdr.checksum    = htonl(compute_checksum((const uint8_t *)message, msg_len)); // 32-bit checksum in network byte order.

    // Copy the fully populated header struct into the frame buffer immediately after the
    // 14-byte Ethernet header.  We use memcpy rather than a pointer cast to avoid undefined
    // behaviour from potentially unaligned writes into the raw byte array.
    memcpy(frame + 14, &hdr, sizeof(hdr));

    // Payload
    // Copy the message string (without null terminator) into the frame right after the custom header.
    // The receiver knows exactly how many bytes to read because we stored msg_len in hdr.payload_len.
    memcpy(frame + 14 + sizeof(CustomHeader), message, msg_len);

    // Calculate the total number of bytes to send: Ethernet header + custom header + payload.
    // We pass this to sendto() so the kernel knows exactly how many bytes of 'frame' to transmit.
    size_t frame_len = 14 + sizeof(CustomHeader) + msg_len;

    // Prepare to send it
    // sockaddr_ll tells sendto() which interface to send on and what destination MAC to use
    // at the link layer.  Unlike IP sockets, AF_PACKET sockets don't route — we must specify
    // the outgoing interface explicitly.
    struct sockaddr_ll saddr;

    // Zero the struct before filling it because uninitialized padding fields can confuse the kernel.
    memset(&saddr, 0, sizeof(saddr));

    // Tell the kernel which physical interface to transmit the frame on.
    saddr.sll_ifindex  = ifindex;

    // Set the hardware address length to 6 bytes (standard Ethernet MAC length).
    // The kernel uses sll_halen to know how many bytes of sll_addr are meaningful.
    saddr.sll_halen    = 6;

    // Copy the destination MAC address into the sockaddr_ll so the kernel can use it
    // when constructing the outgoing frame at the driver level.
    memcpy(saddr.sll_addr, dest_mac, 6);

    // Send the complete raw frame.
    // sendto() returns the number of bytes actually sent, or -1 on error.
    // The cast to (struct sockaddr*) is required because sendto() takes the generic base type;
    // the kernel re-casts it INTERNALLY using the sll_family field.
    // Flags argument is 0 — we don't need MSG_DONTWAIT or any other special behavior here.
    
    /*Prototype:
    ssize_t sendto(int sockfd,
               const void *buf,
               size_t len,
               int flags,
               const struct sockaddr *dest_addr,
               socklen_t addrlen);
    Arguments:

    sockfd — socket file descriptor (created with socket()).
    buf — pointer to data to send.
    len — number of bytes to send from buf.
    flags — message flags (e.g., 0, MSG_DONTWAIT, MSG_NOSIGNAL).
    dest_addr — pointer to destination address (cast to struct sockaddr*). 
    For connected sockets this can be NULL on some systems, but normally used for datagram sockets (UDP).

    addrlen — length of the structure pointed to by dest_addr (e.g., sizeof(struct sockaddr_in)).
  
  
    Return and errors*

    Returns number of bytes sent on success; -1 on error and errno is set (e.g., EWOULDBLOCK, EINTR, EDESTADDRREQ).
    
    
    Minimal UDP example in C:
    struct sockaddr_in dst = { .sin_family = AF_INET,
                           .sin_port = htons(12345),
                           .sin_addr.s_addr = inet_addr("192.0.2.1") };
    const char *msg = "hello";
    ssize_t n = sendto(sockfd, msg, strlen(msg), 0,
                   (struct sockaddr*)&dst, sizeof(dst));
    Use send() for connected sockets when you don't need to specify dest_addr each call.*
    */
    ssize_t sent = sendto(fd, frame, frame_len, 0,
                          (struct sockaddr *)&saddr, sizeof(saddr));
    if (sent < 0) { perror("sendto"); exit(1); }

    // Confirm the send succeeded by printing how many bytes were transmitted and
    // a summary of the frame's key header fields so the user can cross-check with the receiver.
    // %zd is the correct format specifier for ssize_t (a signed size type).
    printf("Sent %zd bytes | seq=1 | msg_type=DATA | payload=\"%s\"\n", sent, message);

    // Release the socket file descriptor back to the OS.
    // Always close file descriptors when you're done, failing to do so leaks them for the
    // lifetime of the process, and on long-running programs that eventually exhausts the FD limit.
    close(fd);

    // Exit signalling the program finished without errors.
    exit(EXIT_SUCCESS);
}
