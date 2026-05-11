//! firmware/src/main.rs : bare metal entry point.
//!
//! This replaces both sender/main.rs and receiver/main.rs from the Linux version.
//! On embedded there's one firmware image; the device is both sender and receiver.

//! Please see https://docs.rust-embedded.org/book/ for review

// These two replace `fn main()` in the traditional sense:
#![no_std]   // no standard library, only core
#![no_main]  // no Rust runtime entry point, cortex-m-rt provides our own

// ── Panic handler ─────────────────────────────────────────────────────────────
// On Linux: an unhandled panic unwinds the stack and prints to stderr.
// On bare metal: there's no stderr, no OS, nowhere to unwind TO.
// panic-halt defines the #[panic_handler] for us: disable interrupts, loop forever.
// Your debugger will show you the call stack at the halt point.
use panic_halt as _;

use cortex_m_rt::entry; // the #[entry] attribute, marks our real entry point

mod eth_driver;
mod receiver;
mod sender;

use eth_driver::MockEthDriver;
use receiver::PollResult;

// ── Compile-time configuration ────────────────────────────────────────────────
// On Linux: runtime env vars (INTERFACE=wlp2s0, command-line args).
// On bare metal: no env vars, no argv. Configuration is either:
//   a) compile-time constants like these
//   b) read from hardware (DIP switches, EEPROM, hardwired pins)
//   c) received over a serial/USB config interface at boot

/// This device's MAC address. In production: read from EEPROM or OTP flash.
const MY_MAC:   [u8; 6] = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];

/// Peer's MAC address. In a standard protocol: discovered via a handshake/ARP equivalent.
const PEER_MAC: [u8; 6] = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x02];

// ── Entry point ───────────────────────────────────────────────────────────────
// #[entry] replaces fn main(). The return type `!` means "never returns",
// embedded firmware loops forever, there's nothing to return to.
//
// cortex-m-rt calls this after setting up the stack, zeroing BSS, and copying
// .data from flash to RAM. Equivalent to what the C runtime (crt0.s) did.

#[entry]
fn main() -> ! {
    // ── Hardware initialisation ───────────────────────────────────────────────
    // In a real project this section would use your board's HAL crate:
    //
    //   let dp   = stm32f4xx_hal::pac::Peripherals::take().unwrap();
    //   let rcc  = dp.RCC.constrain();
    //   let clks = rcc.cfgr.sysclk(168.MHz()).freeze();
    //   let gpioa = dp.GPIOA.split();
    //   let spi  = Spi::new(dp.SPI1, (sck, miso, mosi), MODE_0, 10.MHz(), &clks);
    //   let mut eth = Enc28j60::new(spi, cs_pin, MY_MAC);
    //
    // For now we use MockEthDriver so the logic compiles and can be tested
    // without physical hardware or a specific board HAL.

    let mut eth = MockEthDriver::new(MY_MAC);

    // ── Send an initial frame ─────────────────────────────────────────────────
    // On Linux: the sender binary was a separate process you ran manually.
    // On embedded: we decide when to send in firmware logic.
    let message = b"hello from embedded";
    match sender::send_data(&mut eth, &PEER_MAC, message, 1) {
        Ok(n)  => { let _ = n; /* log: sent n bytes */ }
        Err(e) => { let _ = e; /* log: send failed with e */ }
        // On real hardware you'd use RTIC's log!, defmt::info!(), or an RTT logger:
        //   defmt::info!("sent {} bytes", n);
        //   defmt::error!("send failed: {:?}", e);
    }

    // ── Main loop ─────────────────────────────────────────────────────────────
    // Linux receiver: loop { recvfrom(...)  }  — blocking
    // Embedded:       loop { poll() }          — non-blocking, returns immediately
    //
    // A sequence counter for outbound frames — the Linux sender hardcoded seq=1.
    let mut seq: u16 = 1;

    loop {
        // Poll for an incoming frame
        match receiver::poll(&mut eth) {
            PollResult::NoFrame => {
                // Nothing waiting. In real firmware you'd either:
                //   a) sleep until an interrupt fires: cortex_m::asm::wfi()
                //   b) do other work (read sensors, update display, etc.)
                cortex_m::asm::nop(); // placeholder — don't spin at full speed for nothing
            }
            PollResult::Processed { seq: rx_seq, .. } => {
                // We received and ACKed a frame. Maybe send another:
                seq = seq.wrapping_add(1);
                let reply = b"got your frame";
                let _ = sender::send_data(&mut eth, &PEER_MAC, reply, seq);
                let _ = rx_seq; // use this for ordering/duplicate detection in real code
            }
            PollResult::Rejected => {
                // Malformed or wrong-checksum frame : log and continue.
                defmt::warn!("rejected frame");
            }
        }

        // In a real RTOS (RTIC, embassy) you wouldn't have a bare loop{} here.
        // You'd have async tasks or interrupt-driven handlers. But for learning,
        // this is the direct equivalent of the Linux receiver's loop{}.
    }
}
