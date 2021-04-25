//! Universal Asynchronous Receiver Transmitter (UART)
// See [Chapter 4 Section 2](https://datasheets.raspberrypi.org/rp2040/rp2040_datasheet.pdf) for more details

use core::convert::Infallible;
use core::ops::Deref;
use embedded_time::rate::Baud;
use embedded_time::rate::Hertz;
use embedded_time::fixed_point::FixedPoint;
use nb::Error::WouldBlock;
use rp2040_pac::{
    uart0::uartlcr_h::W as UART_LCR_H_Writer
};


/// State of the UART Peripheral.
pub trait State {}

/// Trait to handle both underlying devices (UART0 & UART1)
pub trait UARTDevice: Deref<Target = rp2040_pac::uart0::RegisterBlock> {}

impl UARTDevice for rp2040_pac::UART0 {}
impl UARTDevice for rp2040_pac::UART1 {}

/// UART is enabled.
pub struct Enabled;

/// UART is disabled.
pub struct Disabled;

impl State for Enabled {}
impl State for Disabled {}

/// Data bits
pub enum DataBits {
    /// 5 bits
    Five,
    /// 6 bits
    Six,
    /// 7 bits
    Seven,
    /// 8 bits
    Eight
}

/// Stop bits
pub enum StopBits {
    /// 1 bit
    One,

    /// 2 bits
    Two
}

/// Parity
/// The "none" state of parity is represented with the Option type (None).
pub enum Parity {
    /// Odd parity
    Odd,

    /// Even parity
    Even
}


/// A struct holding the configuration for an UART device.
pub struct UARTConfig {
    baudrate: Baud,
    data_bits: DataBits,
    stop_bits: StopBits,
    parity: Option<Parity>
}

/// Common configurations for UART.
pub mod common_configs {
    use super::{ UARTConfig, DataBits, StopBits };
    use embedded_time::rate::Baud;

    /// 9600 baud, 8 data bits, no parity, 1 stop bit
    pub const _9600_8_N_1: UARTConfig = UARTConfig {
        baudrate: Baud(9600),
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: None
    };

    /// 19200 baud, 8 data bits, no parity, 1 stop bit
    pub const _19200_8_N_1: UARTConfig = UARTConfig {
        baudrate: Baud(19200),
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: None
    };

    /// 38400 baud, 8 data bits, no parity, 1 stop bit
    pub const _38400_8_N_1: UARTConfig = UARTConfig {
        baudrate: Baud(38400),
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: None
    };

    /// 57600 baud, 8 data bits, no parity, 1 stop bit
    pub const _57600_8_N_1: UARTConfig = UARTConfig {
        baudrate: Baud(57600),
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: None
    };

    /// 115200 baud, 8 data bits, no parity, 1 stop bit
    pub const _115200_8_N_1: UARTConfig = UARTConfig {
        baudrate: Baud(115200),
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: None
    };
}

/// An UART Peripheral based on an underlying UART device.
pub struct UARTPeripheral<S: State, D: UARTDevice> {
    device: D,
    _state: S,
    config: UARTConfig,
    effective_baudrate: Baud
}

impl<S: State, D: UARTDevice> UARTPeripheral<S, D> {
    fn transition<To: State>(self, state: To) -> UARTPeripheral<To, D> {
        UARTPeripheral {
            device: self.device,
            config: self.config,
            effective_baudrate: self.effective_baudrate,
            _state: state
        }
    }

    /// Releases the underlying device.
    pub fn free(self) -> D{
        self.device
    }
}

impl<D: UARTDevice> UARTPeripheral<Disabled, D> {

    /// Enables the provided UART device with the given configuration.
    pub fn enable(mut device: D, config: UARTConfig, frequency: Hertz) -> UARTPeripheral<Enabled, D> {

        let effective_baudrate = configure_baudrate(&mut device, &config.baudrate, &frequency);

        // Enable the UART, both TX and RX
        device.uartcr.write(|w| {
            w.uarten().set_bit();
            w.txe().set_bit();
            w.rxe().set_bit();
            w
        });

        device.uartlcr_h.write(|w| {
            w.fen().set_bit();

            set_format(w, &config.data_bits, &config.stop_bits, &config.parity);
            w
        });

        device.uartdmacr.write(|w| {
            w.txdmae().set_bit();
            w.rxdmae().set_bit();
            w
        });

        UARTPeripheral {
            device, config, effective_baudrate, _state: Enabled
        }
    }
}

impl<D: UARTDevice> UARTPeripheral<Enabled, D> {

    /// Disable this UART Peripheral, falling back to the Disabled state.
    pub fn disable(self) -> UARTPeripheral<Disabled, D> {
        self.transition(Disabled)
    }

    fn uart_is_writable(&self) -> bool {
        self.device.uartfr.read().txff().bit_is_clear()
    }

    fn uart_is_readable(&self) -> bool {
        self.device.uartfr.read().rxfe().bit_is_clear()
    }

    /// Writes bytes to the UART.
    /// This function writes as long as it can. As soon that the FIFO is full, if :
    /// - 0 bytes were written, a WouldBlock Error is returned
    /// - some bytes were written, it is deemed to be a success
    /// Upon success, the number of written bytes is returned.
    pub fn write(&self, data: &[u8]) -> nb::Result<usize, Infallible> {

        let mut bytes_written = 0;

        for c in data {

            if !self.uart_is_writable() {
                if bytes_written == 0 {
                    return Err(WouldBlock)
                }
                else {
                    return Ok(bytes_written)
                }
            }

            bytes_written += 1;

            self.device.uartdr.write(|w| unsafe {
                w.data().bits(*c);
                w
            })
        }
        Ok(bytes_written)
    }

    /// Reads bytes from the UART.
    /// This function reads as long as it can. As soon that the FIFO is empty, if :
    /// - 0 bytes were read, a WouldBlock Error is returned
    /// - some bytes were read, it is deemed to be a success
    /// Upon success, the number of read bytes is returned.
    pub fn read(&self, buffer: &mut [u8]) -> nb::Result<usize, Infallible> {

        let mut bytes_read = 0;

        Ok(loop {
            if !self.uart_is_readable() {
                if bytes_read == 0 {
                    return Err(WouldBlock)
                }
                else {
                    return Ok(bytes_read)
                }
            }

            if bytes_read < buffer.len() {
                buffer[bytes_read] = self.device.uartdr.read().data().bits();
                bytes_read += 1;
            }
            else {
                break bytes_read;
            }
        })
    }

    /// Writes bytes to the UART.
    /// This function blocks until the full buffer has been sent.
    pub fn write_full_blocking(&self, data: &[u8]) {

        let mut offset = 0;

        while offset != data.len() {
            offset += match self.write(&data[offset..]) {
                Ok(written_bytes) => {
                    written_bytes
                }

                Err(WouldBlock) => continue,

                Err(_) => unreachable!()
            };
        }
    }

    /// Reads bytes from the UART.
    /// This function blocks until the full buffer has been received.
    pub fn read_full_blocking(&self, buffer: &mut [u8]) {
        let mut offset = 0;

        while offset != buffer.len() {
            offset += match self.read(&mut buffer[offset..]) {
                Ok(bytes_read) => {
                    bytes_read
                }

                Err(WouldBlock) => continue,

                Err(_) => unreachable!()
            };
        }
    }

}

/// Baudrate configuration. Code loosely inspired from the C SDK.
fn configure_baudrate(device: &mut dyn UARTDevice, wanted_baudrate: &Baud, frequency: &Hertz) -> Baud {

    let frequency = *frequency.integer();

    let baudrate_div = 8 * frequency / *wanted_baudrate.integer();

    let (baud_ibrd, baud_fbrd) = match (baudrate_div >> 7, ((baudrate_div & 0x7F) + 1) / 2) {

        (0, _) => (1, 0),

        (ibrd, _) if ibrd >= 65535 => (65535, 0),

        (ibrd, fbrd) => (ibrd, fbrd)
    };

    // Load PL011's baud divisor registers
    device.uartibrd.write(|w| unsafe {
        w.baud_divint().bits(baud_ibrd as u16);
        w
    });
    device.uartfbrd.write(|w| unsafe {
        w.baud_divfrac().bits(baud_fbrd as u8);
        w
    });

    // PL011 needs a (dummy) line control register write to latch in the
    // divisors. We don't want to actually change LCR contents here.
    device.uartlcr_h.write(|w| {
        w
    });

    Baud((4 * frequency) / (64 * baud_ibrd + baud_fbrd))
}


/// Format configuration. Code loosely inspired from the C SDK.
fn set_format<'w>(w: &'w mut UART_LCR_H_Writer, data_bits: &DataBits, stop_bits: &StopBits, parity: &Option<Parity>) -> &'w mut UART_LCR_H_Writer {

    match parity {
        Some(p) => {
            w.pen().set_bit();
            match p {
                Parity::Odd => w.eps().bit(false),
                Parity::Even => w.eps().set_bit()
            };
        },
        None => { w.pen().bit(false); }
    };

    unsafe { w.wlen().bits(
        match data_bits {
                DataBits::Five => { 0b00 }
                DataBits::Six => { 0b01 }
                DataBits::Seven => { 0b10 }
                DataBits::Eight => { 0b11 }
            }
        )
    };

    match stop_bits {
        StopBits::One => w.stp2().bit(false),
        StopBits::Two => w.stp2().set_bit()
    };

    w
}
