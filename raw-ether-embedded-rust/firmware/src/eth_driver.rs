//! eth_driver : hardware abstraction for raw Ethernet.
//!
//! On Linux, the OS gave us an AF_PACKET socket as the abstraction.
//! On bare metal there is no OS. Instead we define a TRAIT that describes
//! what we need from any Ethernet hardware, and write our protocol logic
//! against that trait. The actual hardware driver implements the trait.
//!
//! This is the Rust embedded pattern for hardware independence:
//!   trait = the contract  (what we need)
//!   struct = the implementation  (specific chip: ENC28J60, W5500, STM32 MAC...)
//!
//! Real-world crates that would implement this trait:
//!   - enc28j60  (SPI Ethernet chip, very common on hobbyist boards)
//!   - w5500-ll  (SPI Ethernet chip with hardware TCP/IP stack)
//!   - stm32f4xx-hal Ethernet driver (built-in MAC on higher-end STM32s)
//!   - embassy-net  (async networking stack, works with the above)

// ── Error type ────────────────────────────────────────────────────────────────
// On Linux: std::io::Error (wraps errno).
// On embedded: our own enum. No OS, no errno : WE define what can go wrong.
// `#[derive(Debug)]` is still available in no_std via core::fmt::Debug.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthError {
    /// Transmit buffer full — hardware not ready to send.
    TxBusy,
    /// No frame waiting to be received.
    RxEmpty,
    /// The provided buffer is too small for the received frame.
    BufferTooSmall,
    /// Hardware fault (link down, SPI error, etc.)
    HardwareFault,
}

// ── The trait ────────────────────────────────────────────────────────────────
// This is what replaced the entire AF_PACKET socket setup in sender.c and
// receiver.c: the ioctl() calls, the sockaddr_ll setup, the bind(), all of it.
// The trait implementor (real hardware driver) does all of that work.

pub trait EthernetMac {
    /// Send a raw Ethernet frame (everything including the Ethernet header).
    /// Returns the number of bytes sent, or an error.
    ///
    /// Linux equivalent:
    ///   sendto(fd, frame.as_ptr(), frame_len, 0, &saddr, sizeof(saddr))
    fn send(&mut self, frame: &[u8]) -> Result<usize, EthError>;

    /// Check if a frame is available and copy it into `buf`.
    /// Returns the number of bytes received, or EthError::RxEmpty if nothing
    /// is waiting.
    ///
    /// This is non-blocking — it returns immediately with RxEmpty rather than
    /// blocking like recvfrom() did on Linux. In embedded you either poll in a
    /// loop, or use interrupts + a queue (the embassy-net approach).
    ///
    /// Linux equivalent:
    ///   recvfrom(fd, buf.as_mut_ptr(), buf.len(), 0, null, null)
    ///   (but non-blocking — set O_NONBLOCK or use MSG_DONTWAIT)
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, EthError>;

    /// Return this interface's MAC address.
    ///
    /// Linux equivalent:
    ///   ioctl(fd, SIOCGIFHWADDR, &ifr)   → ifr.ifr_hwaddr.sa_data[0..6]
    fn mac_address(&self) -> [u8; 6];
}

// ── MockEthDriver ─────────────────────────────────────────────────────────────
// A software-only implementation of EthernetMac for testing on a host machine
// or in QEMU without real hardware. It has an internal loopback ring buffer:
// frames sent via send() can be read back via recv(). Useful for unit tests.
//
// In a real project this lives under #[cfg(test)] or a "mock" feature flag.

/// Internal frame storage: one frame at a time (a real driver would have a ring
/// buffer with multiple slots). Fixed size, no heap needed.
pub struct MockEthDriver {
    mac: [u8; 6],
    /// Pending received frame, if any.
    rx_frame: [u8; common::MAX_FRAME_SIZE],
    /// How many bytes of rx_frame are valid. 0 = no frame waiting.
    rx_len: usize,
}

impl MockEthDriver {
    pub fn new(mac: [u8; 6]) -> Self {
        Self {
            mac,
            rx_frame: [0u8; common::MAX_FRAME_SIZE],
            rx_len: 0,
        }
    }

    /// Inject a frame as if it arrived from the network, for testing.
    pub fn inject_frame(&mut self, frame: &[u8]) {
        let len = frame.len().min(common::MAX_FRAME_SIZE);
        self.rx_frame[..len].copy_from_slice(&frame[..len]);
        self.rx_len = len;
    }
}

impl EthernetMac for MockEthDriver {
    fn send(&mut self, frame: &[u8]) -> Result<usize, EthError> {
        // In loopback mode: sending a frame also makes it receivable.
        // A real driver would push bytes to the chip's TX buffer via SPI/RMII.
        let len = frame.len().min(common::MAX_FRAME_SIZE);
        self.rx_frame[..len].copy_from_slice(&frame[..len]);
        self.rx_len = len;
        Ok(len)
    }

    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, EthError> {
        if self.rx_len == 0 {
            return Err(EthError::RxEmpty);
        }
        if buf.len() < self.rx_len {
            return Err(EthError::BufferTooSmall);
        }
        let len = self.rx_len;
        buf[..len].copy_from_slice(&self.rx_frame[..len]);
        self.rx_len = 0; // consumed
        Ok(len)
    }

    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }
}
