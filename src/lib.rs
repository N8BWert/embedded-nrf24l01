// Copyright 2018, Astro <astro@spaceboyz.net>
//
// Licensed under the Apache License, Version 2.0 <LICENSE>. This file
// may not be copied, modified, or distributed except according to
// those terms.

//! nRF24L01+ driver for use with [embedded-hal](https://crates.io/crates/embedded-hal)

#![warn(missing_docs, unused)]


#![no_std]
#[macro_use]
extern crate bitfield;

use core::fmt;
use core::fmt::Debug;

use embedded_hal::blocking::spi::Transfer as SpiTransfer;
use embedded_hal::digital::v2::OutputPin;

mod config;
pub use crate::config::{Configuration, CrcMode, DataRate};
pub mod setup;

mod registers;
use crate::registers::{Config, Register, SetupAw, Status, FifoStatus, CD};
mod command;
use crate::command::{Command, ReadRegister, WriteRegister, ReadRxPayloadWidth, ReadRxPayload, WriteTxPayload, FlushTx};
mod payload;
pub use crate::payload::Payload;
mod error;
pub use crate::error::Error;

mod device;
pub use crate::device::Device;
mod rx;
pub use crate::rx::Rx;
mod tx;
pub use crate::tx::Tx;
mod mode;
pub use crate::mode::{Mode, ChangeModes};

/// Number of RX pipes with configurable addresses
pub const PIPES_COUNT: usize = 6;
/// Minimum address length
pub const MIN_ADDR_BYTES: usize = 2;
/// Maximum address length
pub const MAX_ADDR_BYTES: usize = 5;

/// Driver for the nRF24L01+
///
/// Never deal with this directly. Instead, you store one of the following types:
///
/// * [`StandbyMode<D>`](struct.StandbyMode.html)
/// * [`RxMode<D>`](struct.RxMode.html)
/// * [`TxMode<D>`](struct.TxMode.html)
///
/// where `D: `[`Device`](trait.Device.html)
pub struct NRF24L01<E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8>> {
    ce: CE,
    csn: CSN,
    spi: SPI,
    config: Config,
    mode: Mode,
}

impl<E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> fmt::Debug
    for NRF24L01<E, CE, CSN, SPI>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NRF24L01")
    }
}

impl<E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug>
    NRF24L01<E, CE, CSN, SPI>
{
    /// Construct a new driver instance.
    pub fn new(mut ce: CE, mut csn: CSN, spi: SPI) -> Result<Self, Error<SPIE>> {
        ce.set_low().unwrap();
        csn.set_high().unwrap();

        // Reset value
        let mut config = Config(0b0000_1000);
        config.set_mask_rx_dr(false);
        config.set_mask_tx_ds(false);
        config.set_mask_max_rt(false);
        let mut device = NRF24L01 {
            ce,
            csn,
            spi,
            config,
            mode: Mode::Standby,
        };

        match device.is_connected() {
            Err(e) => return Err(e),
            Ok(false) => return Err(Error::NotConnected),
            _ => {}
        }

        // TODO: activate features?

        match device.update_config(|config| config.set_pwr_up(true)) {
            Ok(_) => Ok(device),
            Err(err) => Err(err),
        }
    }

    /// Reads and validates content of the `SETUP_AW` register.
    pub fn is_connected(&mut self) -> Result<bool, Error<SPIE>> {
        let (_, setup_aw) = self.read_register::<SetupAw>()?;
        let valid = setup_aw.aw() <= 3;
        Ok(valid)
    }
}

impl<E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> Device
    for NRF24L01<E, CE, CSN, SPI>
{
    type Error = Error<SPIE>;

    fn ce_enable(&mut self) {
        self.ce.set_high().unwrap();
    }

    fn ce_disable(&mut self) {
        self.ce.set_low().unwrap();
    }

    fn send_command<C: Command>(
        &mut self,
        command: &C,
    ) -> Result<(Status, C::Response), Self::Error> {
        // Allocate storage
        let mut buf_storage = [0; 33];
        let len = command.len();
        let buf = &mut buf_storage[0..len];
        // Serialize the command
        command.encode(buf);

        // SPI transaction
        self.csn.set_low().unwrap();
        let transfer_result = self.spi.transfer(buf).map(|_| {});
        self.csn.set_high().unwrap();
        // Propagate Err only after csn.set_high():
        transfer_result?;

        // Parse response
        let status = Status(buf[0]);
        let response = C::decode_response(buf);

        Ok((status, response))
    }

    fn write_register<R: Register>(&mut self, register: R) -> Result<Status, Self::Error> {
        let (status, ()) = self.send_command(&WriteRegister::new(register))?;
        Ok(status)
    }

    fn read_register<R: Register>(&mut self) -> Result<(Status, R), Self::Error> {
        self.send_command(&ReadRegister::new())
    }

    fn update_config<F, R>(&mut self, f: F) -> Result<R, Self::Error>
    where
        F: FnOnce(&mut Config) -> R,
    {
        // Mutate
        let old_config = self.config.clone();
        let result = f(&mut self.config);

        if self.config != old_config {
            let config = self.config.clone();
            self.write_register(config)?;
        }
        Ok(result)
    }
}

impl<E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> ChangeModes
    for NRF24L01<E, CE, CSN, SPI>
{
    type Error = Error<SPIE>;

    fn to_standby(&mut self) -> Result<(), Self::Error> {
        match self.mode {
            Mode::Standby => Ok(()),
            Mode::PowerDown => match self.update_config(|config| config.set_pwr_up(true)) {
                Ok(()) => {
                    self.mode = Mode::Standby;
                    return Ok(());
                },
                Err(err) => Err(err),
            },
            Mode::Rx | Mode::Tx => {
                self.ce_disable();
                self.mode = Mode::Standby;
                return Ok(());
            },
        }
    }

    fn to_power_down(&mut self) -> Result<(), Self::Error> {
        match self.mode {
            Mode::Standby => match self.update_config(|config| config.set_pwr_up(false)) {
                Ok(_) => {
                    self.mode = Mode::PowerDown;
                    return Ok(());
                },
                Err(err) => Err(err),
            },
            Mode::PowerDown => Ok(()),
            Mode::Rx | Mode::Tx => {
                match self.to_standby() {
                    Ok(_) => self.to_power_down(),
                    Err(err) => Err(err),
                }
            },
        }
    }

    fn to_rx(&mut self) -> Result<(), Self::Error> {
        match self.mode {
            Mode::Standby => {
                match self.update_config(|config| config.set_prim_rx(true)) {
                    Ok(_) => {
                        self.ce_enable();
                        return Ok(());
                    },
                    Err(err) => Err(err),
                }
            },
            Mode::PowerDown | Mode::Tx => match self.to_standby() {
                Ok(_) => self.to_rx(),
                Err(err) => Err(err),
            },
            Mode::Rx => Ok(()),
        }
    }

    fn to_tx(&mut self) -> Result<(), Self::Error> {
        match self.mode {
            Mode::Standby => {
                match self.update_config(|config| config.set_prim_rx(false)) {
                    Ok(_) => Ok(()),
                    Err(err) => Err(err),
                }
            },
            Mode::PowerDown | Mode::Rx => match self.to_standby() {
                Ok(_) => self.to_tx(),
                Err(err) => Err(err),
            },
            Mode::Tx => Ok(()),
        }
    }
}

impl<E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> Rx
    for NRF24L01<E, CE, CSN, SPI>
{
    type Error = Error<SPIE>;

    /// Is there any incoming data to read? Return the pipe number.
    ///
    /// This function acknowledges all interrupts even if there are more received packets, so the
    /// caller must repeat the call until the function returns None before waiting for the next RX
    /// interrupt.
    fn can_read(&mut self) -> Result<Option<u8>, Self::Error> {
        if self.mode != Mode::Rx {
            if let Err(err) = self.to_rx() {
                return Err(err);
            }
        }

        let mut clear = Status(0);
        clear.set_rx_dr(true);
        clear.set_tx_ds(true);
        clear.set_max_rt(true);
        self.write_register(clear)?;

        self.read_register::<FifoStatus>()
            .map(|(status, fifo_status)| {
                if !fifo_status.rx_empty() {
                    Some(status.rx_p_no())
                } else {
                    None
                }
            })
    }

    /// Is an in-band RF signal detected?
    ///
    /// The internal carrier detect signal must be high for 40μs
    /// (NRF24L01+) or 128μs (NRF24L01) before the carrier detect
    /// register is set. Note that changing from standby to receive
    /// mode also takes 130μs.
    fn has_carrier(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Rx {
            if let Err(err) = self.to_rx() {
                return Err(err);
            }
        }

        self.read_register::<CD>()
            .map(|(_, cd)| cd.0 & 1 == 1)
    }

    /// Is the RX queue empty?
    fn rx_queue_empty(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Rx {
            if let Err(err) = self.to_rx() {
                return Err(err);
            }
        }

        self.read_register::<FifoStatus>()
            .map(|(_, fifo_status)| fifo_status.rx_empty())
    }

    /// Is the RX queue full?
    fn rx_queue_is_full(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Rx {
            if let Err(err) = self.to_rx() {
                return Err(err);
            }
        }

        self.read_register::<FifoStatus>()
            .map(|(_, fifo_status)| fifo_status.rx_full())
    }

    /// Read the next received packet
    fn read(&mut self) -> Result<Payload, Self::Error> {
        if self.mode != Mode::Rx {
            if let Err(err) = self.to_rx() {
                return Err(err);
            }
        }

        let (_, payload_width) = self.send_command(&ReadRxPayloadWidth)?;
        let (_, payload) = self.send_command(&ReadRxPayload::new(payload_width as usize))?;
        Ok(payload)
    }
}

impl<E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> Tx
    for NRF24L01<E, CE, CSN, SPI>
{
    type Error = Error<SPIE>;

    fn tx_empty(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Tx {
            if let Err(err) = self.to_tx() {
                return Err(err);
            }
        }

        let (_, fifo_status) = self.read_register::<FifoStatus>()?;
        Ok(fifo_status.tx_empty())
    }

    fn tx_full(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Tx {
            if let Err(err) = self.to_tx() {
                return Err(err);
            }
        }

        let (_, fifo_status) = self.read_register::<FifoStatus>()?;
        Ok(fifo_status.tx_full())
    }

    fn can_send(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Tx {
            if let Err(err) = self.to_tx() {
                return Err(err);
            }
        }

        let full = self.tx_full()?;
        Ok(!full)
    }

    fn send(&mut self, packet: &[u8]) -> Result<(), Self::Error> {
        if self.mode != Mode::Tx {
            if let Err(err) = self.to_tx() {
                return Err(err);
            }
        }

        self.send_command(&WriteTxPayload::new(packet))?;
        self.ce_enable();
        Ok(())
    }

    fn poll_send(&mut self) -> nb::Result<bool, Self::Error> {
        if self.mode != Mode::Tx {
            if let Err(err) = self.to_tx() {
                return core::prelude::v1::Err(nb::Error::Other(err));
            }
        }

        let (status, fifo_status) = self.read_register::<FifoStatus>()?;
        // We need to clear all the TX interrupts whenever we return Ok here so that the next call
        // to poll_send correctly recognizes max_rt and send completion.
        if status.max_rt() {
            // If MAX_RT is set, the packet is not removed from the FIFO, so if we do not flush
            // the FIFO, we end up in an infinite loop
            self.send_command(&FlushTx)?;
            self.clear_tx_interrupts_and_ce()?;
            Ok(false)
        } else if fifo_status.tx_empty() {
            self.clear_tx_interrupts_and_ce()?;
            Ok(true)
        } else {
            self.ce_enable();
            Err(nb::Error::WouldBlock)
        }
    }

    fn clear_tx_interrupts_and_ce(&mut self) -> nb::Result<(), Self::Error> {
        if self.mode != Mode::Tx {
            if let Err(err) = self.to_tx() {
                return core::prelude::v1::Err(nb::Error::Other(err));
            }
        }

        let mut clear = Status(0);
        clear.set_tx_ds(true);
        clear.set_max_rt(true);
        self.write_register(clear)?;

        // Can save power now
        self.ce_disable();

        Ok(())
    }

    fn wait_empty(&mut self) -> Result<(), Self::Error> {
        if self.mode != Mode::Tx {
            if let Err(err) = self.to_tx() {
                return Err(err);
            }
        }

        let mut empty = false;
        while !empty {
            let (status, fifo_status) = self.read_register::<FifoStatus>()?;
            empty = fifo_status.tx_empty();
            if !empty {
                self.ce_enable();
            }

            // TX won't continue while MAX_RT is set
            if status.max_rt() {
                let mut clear = Status(0);
                // If MAX_RT is set, the packet is not removed from the FIFO, so if we do not flush
                // the FIFO, we end up in an infinite loop
                self.send_command(&FlushTx)?;
                // Clear TX interrupts
                clear.set_tx_ds(true);
                clear.set_max_rt(true);
                self.write_register(clear)?;
            }
        }
        // Can save power now
        self.ce_disable();

        Ok(())
    }

    fn observe(&mut self) -> Result<registers::ObserveTx, Self::Error> {
        if self.mode != Mode::Tx {
            if let Err(err) = self.to_tx() {
                return Err(err);
            }
        }

        let (_, observe_tx) = self.read_register()?;
        Ok(observe_tx)
    }
}