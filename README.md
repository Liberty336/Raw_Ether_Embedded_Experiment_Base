# Raw_Ether_Embedded_Experiment_Base
<img width="766" height="589" alt="image" src="https://github.com/user-attachments/assets/acff0e92-f37b-49f4-a771-cab882232c4f" />

Liberty336 wanted its own custom experimental protocol.<br>
Here are base experimental implementations and not complete for "production" use.
<br><br>
It <strong>can</strong> be used out of the box, but is <strong>-not intended for such purposes-</strong>.<br>
It is intended:<br>
  1. To be somewhat of an educational "guide" on how to implement certain concepts in Rust.
  2. To be modified by the user to meet the user's needs and then be deployed.

### Key differences between original Linux C version and Embedded Rust Version
The key differences were:

### Transport
Linux C: AF_PACKET raw socket, sendto()/recvfrom() syscalls<br>
Embedded Rust: EthernetMac trait (hardware abstraction), real chip driver implements it

### Buffers
Linux C: stack array or heap malloc<br>
Embedded Rust: fixed size stack array [u8; MAX_FRAME_SIZE], no heap or no allocator needed

### Configuration
Linux C: -DINTERFACE_NAME compile flag or env vars at runtime<br>
Embedded Rust: compile-time constants, no OS, no env vars, no argv (could've done this in C also)

### Error handling
Linux C: return -1, check errno, call perror()<br>
Embedded Rust: custom error enum, Result<T, EthError>

### Failure/exit
Linux C: process::exit() / exit(1)<br>
Embedded Rust: loop {} or halt — nowhere to exit to

### Panic
Linux C: segfault or undefined behaviour<br>
Embedded Rust: panic-halt (disables interrupts, freezes, debugger shows you where)

### Structure
Linux C: two separate binaries, sender and receiver<br>
Embedded Rust: one firmware image, both roles handled in the main loop
