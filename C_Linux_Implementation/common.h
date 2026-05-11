#ifndef COMMON_H
#define COMMON_H

#include <stdint.h>

// Custom EtherType - not registered with IEEE, Wireshark won't know what it is
// Valid unregistered range is generally 0x88B6, 0x88B7, or anything not in
// https://www.iana.org/assignments/ieee-802-numbers/ieee-802-numbers.xhtml
#define CUSTOM_ETHERTYPE 0x88B6

#define MAX_PAYLOAD 1024

/* Custom protocol header
   This will be the "header" of our packets.
   
   It's important to note that 'packed' must be used or the compiler
   will add bytes for padding.

   using 'packed' ensures the compiler makes our struct exactly as we want it
   byte by byte.
   
   Also, __attribute__ is a GNU C Compiler extension, not ISO C.
   Use the GCC compiler.
*/
typedef struct __attribute__((packed)) {
    uint8_t  version;       // protocol version
    uint8_t  msg_type;      // e.g. 0x01 = data, 0x02 = ack, 0x03 = ping
    uint16_t seq;           // sequence number
    uint16_t payload_len;   // length of data following this header
    uint32_t checksum;      // simple checksum
} CustomHeader;

// Message types
#define MSG_DATA 0x01
#define MSG_ACK  0x02
#define MSG_PING 0x03


// T wanted his own CustomSocket struct to store and work with certain  values because
// struct sockaddr_ll does NOT have our desired pointers to functions
typedef struct {
  // where we store our integer file descriptor for our socket
  unsigned int fd_value;
  // the memory address in hardware from pointer
  unsigned int *memory_address;
  // the MAC address
  const char *mac_address;
  // The socket interface (like: "wlp2s0" or "eth0")
  const char *socket_interface;
  
  // This is declaring that:
  // GetSockInterface is a pointer to a function that takes const char * and returns void
  void (*GetSockInterface)(const char *);
} CustomSocket;

/*

# On the receiving machine (or same machine on loopback for testing):
sudo ./receiver

# On the sending machine:
sudo ./sender eth0 AA:BB:CC:DD:EE:FF "hello world"
# replace AA:BB:CC:DD:EE:FF with the receiver's actual MAC
# replace eth0 with your actual interface name (check with: ip link)


in our case, we want
sudo ./sender wlp2s0 ac:b5:7d:06:40:49 "hello world"

MAC addresses ARE NOT CASE SENSITIVE

and for receiver we want
gcc -std=gnu17 -Wall -Wextra -pthread -DINTERFACE_NAME="wlp2s0" -fsanitize=address,bounds-strict,undefined receiver.c -o receiver

sudo ./receiver

*take note when we compiled with gcc, we gave it the -DINTERFACE_NAME flag to tell gcc which interface we were listening on
*/
#endif
