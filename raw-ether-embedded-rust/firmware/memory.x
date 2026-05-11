/* memory.x — memory map for the target MCU.
 *
 * This is the embedded equivalent of nothing in the Linux version — Linux
 * handles memory layout automatically. On bare metal YOU must tell the linker
 * where things live.
 *
 * These values are for an STM32F411 (common on "Black Pill" boards, ~$3).
 * Change ORIGIN and LENGTH to match your actual chip's datasheet.
 *
 * Common examples:
 *   STM32F103 (Blue Pill):  FLASH 64K  @ 0x08000000, RAM 20K  @ 0x20000000
 *   STM32F411 (Black Pill): FLASH 512K @ 0x08000000, RAM 128K @ 0x20000000
 *   RP2040 (Pi Pico):       FLASH 2M   @ 0x10000000, RAM 264K @ 0x20000000
 *   nRF52840:               FLASH 1M   @ 0x00000000, RAM 256K @ 0x20000000
 */
MEMORY
{
    FLASH : ORIGIN = 0x08000000, LENGTH = 512K
    RAM   : ORIGIN = 0x20000000, LENGTH = 128K
}
