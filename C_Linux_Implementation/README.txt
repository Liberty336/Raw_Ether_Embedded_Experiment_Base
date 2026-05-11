Build and run:
To compile and build receiver.c type
gcc -std=gnu17 -pthread -O2 receiver.c -o receiver

for the sender, type
gcc -std=gnu17 -pthread -O2 sender.c -o sender


# On the receiving machine (or same machine on loopback for testing):
sudo ./receiver

# On the sending machine:
sudo ./sender eth0 AA:BB:CC:DD:EE:FF "hello world"
# replace AA:BB:CC:DD:EE:FF with the receiver's actual MAC
# replace eth0 with your actual interface name (check with: ip link)


/* A few things to note as you build on this:

__attribute__((packed)) on the struct is important
without it, the compiler may insert padding bytes between fields, 
which would break parsing on the other end.

htons()/htonl() converts values to network byte order (big-endian). 
Always do this for any multi-byte field you put on the wire, 
or the receiver will read garbled numbers.

The msg_type field in CustomHeader is where you'd start building out real protocol behavior
request/response, chunked transfers, session IDs, etc.

SOCK_RAW requires root (sudo) because it bypasses the kernel's normal protocol handling.
That's expected.

To test on a single machine, you can use the loopback-adjacent trick of sending
to your own MAC on an interface or spin up two network namespaces.
*/
