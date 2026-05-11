/***********************************************************************************************************/
/* Program name:  Custom_Ether_Receiver.c     (or receiver.c)                                              */         
/* Program purpose: To receive our custom protocol packets, decode them, and give info on our socket       */
/* Date created:  Thursday, April 22nd, 2026 4-22-2026                                                     */
/*                                                                                                         */
/*                                                                                                         */
/* Compile Options:                                                                                        */
/*                                                                                                         */
/* gcc -std=gnu17 -Wall -Wextra -g -Og -fno-omit-frame-pointer -fstack-protector-strong \                  */
/*     -D_FORTIFY_SOURCE=2 -fsanitize=address,leak,undefined -pthread receiver.c -o receiver               */
/*                                                                                                         */
/*                                                                                                         */
/*       You can also tell the compiler to use the "wlp2s0" interface with the following:                  */
/* gcc -std=gnu17 -Wall -Wextra -g -Og -fno-omit-frame-pointer -fstack-protector-strong \                  */
/*     -DINTERFACE_NAME='"wlp2s0"' -D_FORTIFY_SOURCE=2  \                                                  */
/*     -fsanitize=address,leak,undefined -pthread receiver.c -o receiver                                   */
/*                                                                                                         */
/*                                                                                                         */
/*    INTENDED TO BE COMPILED WITH GCC ON DEBIAN GNU/LINUX!!!!                                             */
/*                                                                                                         */
/*    IF YOU DON'T SPECIFY WHICH INTERFACE TO LISTEN ON VIA THE COMPILER, THEN YOU MUST                    */
/*    #define INTERFACE_NAME                                                                               */
/*    IN YOUR CODE                                                                                         */
/***********************************************************************************************************/

// can also compile by adding an interface by setting an interface value to -DINTERFACE_NAME
// gcc -std=gnu17 -Wall -Wextra -g -Og -fno-omit-frame-pointer -fstack-protector-strong -DINTERFACE_NAME='"wlp2s0"' -D_FORTIFY_SOURCE=2 -fsanitize=address,leak,undefined -pthread receiver.c -o receiver

// Find your interfaces on linux with `ip a` command
// "eth0" is on some, "wlp2s0" is on newer wifi systems.

// you can verify what your gcc compiler pre-defines with typing
// gcc -dM -E - < /dev/null
// in a terminal

// __GNUC__ is a pre-defined macro that GCC knows about.
// So if __GNUC__ is not defined, then they're not compiling with GCC
// We'll give them an error
#ifndef __GNUC__
#error "This program must be compiled with GCC"
#endif

// if we can't find a linux macro then they're not compiling on linux.
#if !defined(__linux__) && !defined(__linux) && !defined(__gnu_linux__)
#warning "This program isn't being compiled on GNU/Linux, program may not work or have undefined behavior"
#endif





// The #include order matters!
// <sys/types.h> is included first so its size_t definitions take precedence over <stdlib.h>'s definitions
// This code was intended for GNU/Linux, not POSIX.

//Examples of Linux only choices include
// #include <linux/if_packet.h>  // Linux only
// AF_PACKET                     // Linux only
// /proc/self/fd/                // Linux only

// Need to add POSIX compatibility if we want MAC users, or FreeBSD users to be able to use our code


/*
<sys/types.h> is a POSIX header — it's specifically designed to provide the system-level 
typedefs our code relies on (pid_t, uid_t, ssize_t, off_t, etc.)

We DO NOT USE <stddef.h> for our size_t implementation, we use <sys/types.h> because it's specifically defined for POSIX (Linux).

Remember
<stddef.h> is a C standard header — it's about the language itself, not the OS
<sys/types.h> will give us size_t implementations best for our OS

Think of it as:
“All the fundamental typedefs the system wants to guarantee exist before you use higher-level stuff.”

DEFINITIONS

It mainly provides typedefs for core system types used all over POSIX APIs.

🔹 File and filesystem types
typedef ... off_t;    // file sizes / offsets
typedef ... ino_t;    // inode numbers
typedef ... dev_t;    // device IDs
typedef ... mode_t;   // file permissions (rwx bits)
typedef ... nlink_t;  // number of hard links

You’ll see these in things like stat().

🔹 Process and user types
typedef ... pid_t;    // process IDs
typedef ... uid_t;    // user IDs
typedef ... gid_t;    // group IDs

Used with:

getpid(), fork(), getuid(), etc.


🔹 Size and memory-related types
typedef ... size_t;   // unsigned size
typedef ... ssize_t;  // signed size (e.g. read() return)
size_t → for sizes (never negative)
ssize_t → for results that might be -1 (errors)
🔹 Time-related types
typedef ... time_t;   // seconds since epoch
typedef ... clock_t;  // CPU time
🔹 Socket-related types (important for networking)
typedef ... sa_family_t;  // address family (AF_INET, etc.)
typedef ... socklen_t;    // length of socket structures
*/
#include <sys/types.h>




/*
Including <stdint.h> provides fixed-width integer types.
We NEED these so that the sizes are the exact same even if compiled on different devices.

u_int8_t, u_int16_t, u_int32_t

are some of these data types.

u_int8_t is guaranteed to be 8 bits.
u_int32_t is guaranteed to be exactly 32 bits.
*/
#include <stdint.h>



/*
Including <arpa/inet.h> provides declarations for functions and macros that convert Internet (IPv4/IPv6) 
addresses between text and binary forms and handle network/host byte-order conversions.

ntohl, ntohs, htonl, htons (convert 32-bit/16-bit values between host and network byte order)

NOTE:
Network byte order is big-endian, so if your system is little endian, you'll have to convert before sending over network.
use hton* or ntoh* when sending numeric fields over the network.

*/
#include <arpa/inet.h>



/*
<sys/socket.h> defines socklen_t, the socket() function, AF_PACKET and many other things. 
Key contents:

Socket functions:
socket, bind, listen, accept
connect, send, recv, sendto, recvfrom
shutdown, getsockname, getpeername
Address families, socket types, and flags:
AF_INET, AF_INET6, AF_UNIX (address families)
SOCK_STREAM, SOCK_DGRAM, SOCK_RAW (socket types)
SOCK_SEQPACKET, SOCK_NONBLOCK, SOCK_CLOEXEC (flags, where supported)
Socket options and control:
setsockopt, getsockopt
struct sockaddr, struct sockaddr_storage, socklen_t
Message I/O:
struct msghdr, struct iovec, sendmsg, recvmsg
Constants and macros:
SOMAXCONN, SHUT_RD/SHUT_WR/SHUT_RDWR, MSG_* flags (e.g., MSG_DONTWAIT)

Notes:

<sys/socket.h> provides declarations, but protocol specific definitions (e.g., sockaddr_in) come from <netinet/in.h> and <arpa/inet.h>.

Not part of standard C; available on POSIX/Unix-like systems. 

On Windows, use Winsock (winsock2.h) instead.
Example typical server sequence: socket() → bind() → listen() → accept(); client: socket() → connect().*_
*/
#include <sys/socket.h>




/*
<linux/if_packet.h> pulls in Linux-specific declarations used for AF_PACKET (link‑layer / raw packet) sockets. 
Key things it provides:

struct sockaddr_ll — socket address for AF_PACKET (contains sll_family, sll_protocol, sll_ifindex, sll_halen, sll_addr).

PACKET_* constants and types: PACKET_HOST, PACKET_BROADCAST, PACKET_MULTICAST, PACKET_OTHERHOST, etc.

struct tpacket_hdr / tpacket2_hdr and related constants for memory‑mapped packet capture (TPACKET_V1/V2/V3), 
ring buffer control, and PACKET_HDRLEN/TPACKET_ALIGN helpers.

struct packet_mreq and PACKET_ADD_MEMBERSHIP / PACKET_DROP_MEMBERSHIP for multicast/group membership on interfaces.

ioctl/setsockopt/getsockopt option constants specific to packet sockets (e.g., PACKET_RX_RING, PACKET_TX_RING).

Macros and helpers for working with link‑layer sockets.

Notes:

This header is Linux-only (not portable).

AF_PACKET sockets let you receive/send raw Ethernet frames (you typically need CAP_NET_RAW or root).

Combine it with <sys/socket.h> and <net/ethernet.h> (for ETH_ALEN/ethernet header) when writing link‑layer code.
ETH_ALEN is the byte length of an Ethernet (MAC) hardware address, defined as 6. It comes from the kernel headers (e.g., <linux/if_ether.h>) 

*/
#include <linux/if_packet.h>



/* <net/if.h> provides IF_NAMESIZE (the max interface name buffer size) and if_indextoname() 
(which maps an interface index to its name like "eth0"). 

Without it, the compiler has no idea either exists.
*/
#include <net/if.h>





/* <net/ethernet.h> is needed for raw socket frames, lower level than typical socket programming.
It's important you know these definitions

DEFINITIONS

STRUCTS

struct ether_header {
    u_int8_t  ether_dhost[ETH_ALEN];  // destination MAC
    u_int8_t  ether_shost[ETH_ALEN];  // source MAC
    u_int16_t ether_type;             // protocol type
};

ether_dhost → destination MAC address
ether_shost → source MAC address
ether_type → what protocol is inside (IP, ARP, etc.)


CONSTANTS
#define ETH_ALEN   6     // MAC address length
#define ETH_HLEN   14    // Ethernet header length
#define ETH_DATA_LEN 1500
#define ETH_FRAME_LEN 1514

Ether Types
#define ETHERTYPE_IP     0x0800  // IPv4
#define ETHERTYPE_ARP    0x0806  // ARP
#define ETHERTYPE_IPV6   0x86DD  // IPv6


*/
#include <net/ethernet.h>


/* 
Including <unistd.h> exposes POSIX API declarations for low-level UNIX-like system calls and constants. 

We need these for process control:
    fork(), getpid, getppied, getuid, getuid, getgid, getegid, pause, sleep, etc
We need them for File I/O (POSIX):
    read, write, close
We need them for file system and environment access:
    access, rename, link, symlink, readlink

CONSTANTS 

STDIN_FILENO — 0
STDOUT_FILENO — 1
STDERR_FILENO — 2
*/
#include <unistd.h>



// <stdlib.h> gives us memory functions like malloc(), free(), and exit()
// exit(EXIT_SUCCESS);
// exit(EXIT_FAILURE);
#include <stdlib.h>

// <stdio.h> is for printf(), obviously
#include <stdio.h>

// We get more memory functions with <string.h> like memcpy, memmove, memset, memcmp, memchr

// we need memset to fill a block of memory with a single byte value.
// We can also use memset to flush memory by setting everything to 0.
// ex: memset(&Address, 0, sizeof(&Address)); will set the values in &Address to 0
// You want to set a address to 0 to "flush" it before using it in C

// memset( void *Address_to_memory_pool, int value_to_write, size_t size_of_memory_pool );
#include <string.h>


// MUST INCLUDE common.h!!
// It defines our CUSTOM_ETHERTYPE with the header packets defined in the struct
// if you add this to your compiler's header directory (like /usr/include), then you can use
// #include <common.h>
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
 



int main(void) {

    // create a socket file descriptor using our custom ethertype protocol
    // Please understand the int value of the socket points to a file descriptor
    // and isn't the actual socket "object" so to speak.
    // AF_PACKET → work at the link layer (raw Ethernet frames, not IP/TCP)
    // SOCK_RAW  → receive the full frame including the Ethernet header
    // htons(CUSTOM_ETHERTYPE) → filter for only frames carrying our custom protocol;
    //   htons converts the value to network byte order (big-endian) as the kernel expects
    int Socket_FD = socket(AF_PACKET, SOCK_RAW, htons(CUSTOM_ETHERTYPE));
    
    // if socket failed to create (-1 code returned), print error
    if ((Socket_FD < 0)) {

      // perror prints a human-readable error message to stderr describing the current errno value.
      // it's defined in <stdlib.h>
       perror("socket error"); exit(1);
    }

    // Print the file descriptor integer so we can confirm the OS assigned us a valid FD
    // (FDs 0, 1, 2 are stdin/stdout/stderr; a raw socket FD will normally be 3 or higher)
    printf("File Descriptor: %d\n", Socket_FD);
    
    // Confirm which EtherType filter the socket is listening on.
    // %04X formats the value as a 4-digit, zero-padded uppercase hex number — the conventional
    // notation for EtherType values (e.g. 0x88B5).
    printf("Listening for EtherType 0x%04X frames...\n", CUSTOM_ETHERTYPE);

    // Yes, we are putting a #ifdef here
    // If our INTERFACE_NAME is defined, (which it will be if we assigned it an interface at compile time)
    #ifdef INTERFACE_NAME
    
    
    // Convert the human-readable interface name (e.g. "wlp2s0") to its kernel index number.
    // The kernel uses integer indices internally; if_nametoindex() does the name→index lookup.
    // Returns 0 on failure (unlike most POSIX calls, 0 is the error sentinel here, not -1).
    unsigned int idx = if_nametoindex(INTERFACE_NAME);
    
    if (idx == 0) {
        // if_nametoindex() failed — likely the interface name doesn't exist on this machine.
        // We print to stderr (fd 2) so the diagnostic doesn't mix with normal stdout output.
        fprintf(stderr, "if_nametoindex(%s) failed\n", INTERFACE_NAME);
    } else {
        // Declare and zero-initialise a link-layer socket address structure.
        // We'll populate it to tell bind() exactly which interface and protocol to attach to.
        struct sockaddr_ll bind_addr;
        
        
        // Zero out the structure before filling it in.
        // This is essential in C — uninitialized struct padding bytes can contain garbage,
        // and the kernel will reject or misinterpret a bind() call with unexpected non-zero fields.
        memset(&bind_addr, 0, sizeof(bind_addr));
        
        // Specify the address family: AF_PACKET means link-layer (raw Ethernet) socket.
        bind_addr.sll_family = AF_PACKET;
        
        // Store the EtherType in network byte order so the kernel filters correctly.
        // Without this, the socket would receive all EtherTypes on the interface.
        bind_addr.sll_protocol = htons(CUSTOM_ETHERTYPE);
        
        // Bind to the specific interface index we looked up above.
        // Without this the socket would receive matching frames from ALL interfaces.
        bind_addr.sll_ifindex = idx;
        
        
        // Attach the socket to this specific interface + protocol combination.
        // The cast to (struct sockaddr*) is required because bind() takes the generic base type;
        // the kernel re-casts it back to sockaddr_ll internally using the sll_family field.
        if (bind(Socket_FD, (struct sockaddr*)&bind_addr, sizeof(bind_addr)) < 0)
            perror("bind error");
    }
    #endif
    
    /* EXTRA INFO: query and print socket address / binding details */
    
    // struct sockaddr_ll is a Linux-specific socket address structure used for link-layer (Layer 2) networking
    // basically, working directly with things like Ethernet frames instead of IP/TCP/UDP.
    // sockaddr_ll is defined in <linux/if_packet.h> 
    
    /*
    "ll" stands for "link layer" (link-level). struct sockaddr_ll is the sockaddr for AF_PACKET 
    (raw packet sockets) and holds link‑layer addressing and metadata. 
    
    Key fields:

    sll_family — address family (AF_PACKET)
    sll_protocol — protocol (network byte order)
    sll_ifindex — interface index
    sll_hatype — ARP hardware type
    sll_pkttype — packet type (host/broadcast/etc.)
    sll_halen — hardware address length
    sll_addr — hardware (MAC) address
    Used for sending/receiving raw Ethernet frames tied to a specific interface.
    */
    struct sockaddr_ll sll;
    
    
    // socklen_t is a type used to represent the size (length) of socket address structures.
    // it's defined in <sys.socket.h>
    // It's length is obviously the same as its size, so we assigned length based off size.
    socklen_t salen = sizeof(sll);
    
    // getsockname() returning 0 means the call succeeded, so if it succeeded, we continue
    // Keep in mind many/most POSIX C functions return 0 on success
    if (getsockname(Socket_FD, (struct sockaddr*)&sll, &salen) == 0) {
        /* interface index and name */
        // sll_ifindex holds the kernel's integer ID for the bound interface (0 if unbound).
        int ifindex = sll.sll_ifindex;

        
        // IF_NAMESIZE is defined in <net/if.h>
        // IF_NAMESIZE is the max interface name buffer size
        
        // here we made a character array with a buffer size set to IF_NAMESIZE.
        // We pre-fill with "(none)" so if we never overwrite it (e.g. ifindex == 0),
        // we still print a sensible placeholder rather than garbage.
        char ifname[IF_NAMESIZE] = "(none)";
        
        // if_indextoname() maps an interface index to its name like "eth0".
        // if_indextoname() is defined in <net/if.h>
        // Only attempt the lookup when ifindex is positive — index 0 means "not bound to an interface".
        // if_indextoname returns NULL on failure; leave placeholder
        if (ifindex > 0 && if_indextoname(ifindex, ifname) == NULL) {
        
        
            /* if_indextoname returns NULL on failure; leave placeholder */
            // The lookup failed (e.g. the interface disappeared after binding).
            // Write a descriptive fallback string safely using strncpy so we can't overflow ifname.
            strncpy(ifname, "(unknown)", sizeof(ifname));
            
            // '\0' Is the most important string terminator in C 
            // it also is the null character — a char with the integer value 0.
            // 
            // ifname[sizeof(ifname)-1] = '\0'; sets the last element in the array to '\0'
            // strncpy does NOT guarantee null-termination when the source fills the buffer,
            // so we manually force a terminator at the last byte to prevent reading past the end.
            ifname[sizeof(ifname)-1] = '\0';
        }
        
        
        /* protocol in network byte order in sll.sll_protocol */
        // sll_protocol is stored in network byte order; ntohs converts it to host order for display.
        uint16_t proto = ntohs(sll.sll_protocol);
        
        // Print interface index, resolved name, and protocol as a 4-digit hex value.
        printf("  getsockname: ifindex=%d name=%s protocol=0x%04X\n",
               ifindex, ifname, proto);

        /* hardware (MAC) address if present */
        // sll_halen is the hardware address length in bytes (6 for Ethernet).
        // We guard against zero (not set) and values above ETH_ALEN (6) to avoid
        // reading out-of-bounds from the fixed-size sll_addr[8] array.
        if (sll.sll_halen > 0 && sll.sll_halen <= ETH_ALEN) {
            printf("  bound MAC: ");
            
            // Print each MAC address byte as exactly 2 uppercase hex digits.
            // The (unsigned char) cast prevents sign-extension on systems where char is signed —
            // without it, bytes above 0x7F could print as large negative numbers.
            for (int i = 0; i < sll.sll_halen; ++i) {
                printf("%02X", (unsigned char)sll.sll_addr[i]);
                // Print a colon separator between bytes but NOT after the last one,
                // producing the standard MAC format: AA:BB:CC:DD:EE:FF
                if (i + 1 < sll.sll_halen) putchar(':');
            }
            // Move to next line after printing all MAC bytes
            putchar('\n');
        }
    } else {
        perror("getsockname error");
    }

    // Declare a raw byte buffer large enough to hold ANY modern Ethernet frame.
    // 65535 bytes is the maximum size of an IP packet (and comfortably covers jumbo frames),
    // so this buffer will never be too small for a frame we receive via recvfrom().
    uint8_t frame[65536];



    // Main receive loop — runs forever until the process is killed or an unrecoverable error occurs.
    while (1) {
    
    
        // Block until a frame arrives on the socket, then copy it into our buffer.
        // recvfrom() returns the number of bytes actually received, or -1 on error.
        // Passing NULL for the last two arguments means we don't care about the sender's address here.
        ssize_t n = recvfrom(Socket_FD, frame, sizeof(frame), 0, NULL, NULL);
        
        
        // n < 0 signals a system-level error (such as the interface went down).
        // We print the error and continue rather than crashing, so the receiver stays alive.
        if (n < 0) { perror("recvfrom error"); continue; }


        // A valid frame must contain at least a full Ethernet header (ETH_HLEN = 14 bytes)
        // plus our custom application header.  Anything shorter is malformed and we discard it.
        if (n < (ssize_t)(ETH_HLEN + sizeof(CustomHeader))) {
            printf("Frame too short, discarding.\n");
            continue;
        }
        

        // The Ethernet header layout is: [dst MAC 6 bytes][src MAC 6 bytes][EtherType 2 bytes]
        // So the source MAC starts at byte offset 6 from the beginning of the frame.
        uint8_t *src_mac = frame + 6;
        
        
        // Print the sender's MAC address in standard colon-separated hex notation.
        // Each %02X prints a single byte as exactly 2 uppercase hex digits, zero-padded.
        printf("Frame from %02X:%02X:%02X:%02X:%02X:%02X\n",
               src_mac[0], src_mac[1], src_mac[2],
               src_mac[3], src_mac[4], src_mac[5]);


        // Declare a local CustomHeader variable to hold a host-order copy of the received header.
        // We'll fill it via memcpy then byte-swap its multi-byte fields out of network order.
        CustomHeader hdr;
        
        // memcpy() a header read from the Ethernet payload; its multi‑byte fields are encoded in network order.
        // Copy exactly sizeof(hdr) bytes from just after the Ethernet header into our local struct.
        // We use memcpy rather than a direct pointer cast to avoid undefined behaviour from
        // unaligned memory access — the frame buffer has no guaranteed alignment for our struct.
        memcpy(&hdr, frame + ETH_HLEN, sizeof(hdr));
        
        // ntohs converts a 16‑bit value from network byte order (big‑endian) to host byte order.
        // uint16_t ntohs(uint16_t netshort);
        // Ensures multi‑byte numeric values received from the network are interpreted correctly 
        // on the local machine regardless of its endianness.
        // Use htons to convert host→network for sending.
        
        
        // ntohs(hdr.seq) and ntohs(hdr.payload_len) convert 16‑bit values to host order.
        // ntohl converts the 32-bit checksum field from network order to host order.
        // After these three lines, all numeric fields in hdr are safe to use in arithmetic and comparisons.
        hdr.seq         = ntohs(hdr.seq);
        hdr.payload_len = ntohs(hdr.payload_len);
        hdr.checksum    = ntohl(hdr.checksum);



        // Print the decoded header fields so we can trace each received frame during debugging.
        printf("  version=%u  msg_type=0x%02X  seq=%u  payload_len=%u\n",
               hdr.version, hdr.msg_type, hdr.seq, hdr.payload_len);


        // Sanity-check the payload length before we try to read payload bytes.
        // Two conditions guard against bad/malicious frames:
        //   1. payload_len > MAX_PAYLOAD — the sender claims more data than our protocol allows.
        //   2. The total frame size implied by the header exceeds the bytes we actually received (n).
        //      This prevents reading past the end of the 'frame' buffer.
        if (hdr.payload_len > MAX_PAYLOAD ||
            (ssize_t)(ETH_HLEN + sizeof(CustomHeader) + hdr.payload_len) > n) {
            printf("  Invalid payload length, ignoring\n");
            continue;
        }

        // REMEMBER uint8_t *payload is NOT storing all the information of frame + ETH_HLEN + CustomHeader
        // uint8_t *payload is a 8bit POINTER to the first byte in the *payload!
        // it also lets us do byte-wise arithmetic and indexing safely.
        
        // All the information is being stored somewhere else safely (ex: hdr.payload_len stores actual payload length)
        // Point payload at the first byte of the application data, which sits immediately after
        // the Ethernet header and our custom header in the raw frame buffer.
        uint8_t *payload = frame + ETH_HLEN + sizeof(CustomHeader);
        
        
        // Recompute the checksum over the received payload bytes and compare to what the sender sent.
        // If they differ, at least one byte was corrupted in transit — discard the frame.
        uint32_t expected = compute_checksum(payload, hdr.payload_len);
        if (expected != hdr.checksum) {
        // Report both values so we can see how far off the checksum was during debugging.
            printf("  Checksum MISMATCH (got %u, expected %u)\n",
                   hdr.checksum, expected);
            continue;
        }

        printf("  payload: \"%.*s\"\n", (int)hdr.payload_len, payload);
    }


    // Release the socket file descriptor back to the OS.
    // Even though we never reach here in normal operation (the while(1) runs forever),
    // it's good practice to close resources explicitly — and important if the loop is ever refactored.
    // Might add updates to terminate the while loop
    close(Socket_FD);
    



    // Return 0 / EXIT_SUCCESS to the shell, signalling the program finished without errors.
    exit(EXIT_SUCCESS);
}
