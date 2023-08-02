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

pub mod config;
pub use crate::config::{CrcMode, DataRate, NRF24L01Config, NRF24L01Configuration, PALevel, RetransmitConfig};
pub mod setup;

mod registers;
use crate::registers::{Config, Register, SetupAw, Status, FifoStatus, CD, RfCh};
use crate::registers::{RfSetup, EnRxaddr, TxAddr, SetupRetr, EnAa, Dynpd, Feature};
mod command;
use crate::command::{Command, ReadRegister, WriteRegister, ReadRxPayloadWidth, ReadRxPayload, WriteTxPayload, FlushTx, FlushRx};
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
pub struct NRF24L01<'a, E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8>> {
    ce: CE,
    csn: CSN,
    spi: SPI,
    config: Config,
    mode: Mode,
    nrf_config: NRF24L01Config<'a>,
}

impl<'a, E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> fmt::Debug
    for NRF24L01<'a, E, CE, CSN, SPI>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NRF24L01")
    }
}

impl<'a, E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug>
    NRF24L01<'a, E, CE, CSN, SPI>
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
            nrf_config: NRF24L01Config::default(),
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

impl<'a, E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> Device
    for NRF24L01<'a, E, CE, CSN, SPI>
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

impl<'a, E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> ChangeModes
    for NRF24L01<'a, E, CE, CSN, SPI>
{
    type Error = Error<SPIE>;

    fn to_standby(&mut self) -> Result<(), Self::Error> {
        match self.mode {
            Mode::Standby => Ok(()),
            Mode::PowerDown => match self.update_config(|config| config.set_pwr_up(true)) {
                Ok(()) => {
                    self.mode = Mode::Standby;
                    Ok(())
                },
                Err(err) => Err(err),
            },
            Mode::Rx | Mode::Tx => {
                self.ce_disable();
                self.mode = Mode::Standby;
                Ok(())
            },
        }
    }

    fn to_power_down(&mut self) -> Result<(), Self::Error> {
        match self.mode {
            Mode::Standby => match self.update_config(|config| config.set_pwr_up(false)) {
                Ok(_) => {
                    self.mode = Mode::PowerDown;
                    Ok(())
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
                        Ok(())
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

impl<'a, E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> Rx
    for NRF24L01<'a, E, CE, CSN, SPI>
{
    type Error = Error<SPIE>;

    /// Is there any incoming data to read? Return the pipe number.
    ///
    /// This function acknowledges all interrupts even if there are more received packets, so the
    /// caller must repeat the call until the function returns None before waiting for the next RX
    /// interrupt.
    fn can_read(&mut self) -> Result<Option<u8>, Self::Error> {
        if self.mode != Mode::Rx {
            self.to_rx()?;
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
            self.to_rx()?;
        }

        self.read_register::<CD>()
            .map(|(_, cd)| cd.0 & 1 == 1)
    }

    /// Is the RX queue empty?
    fn rx_queue_empty(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Rx {
            self.to_rx()?;
        }

        self.read_register::<FifoStatus>()
            .map(|(_, fifo_status)| fifo_status.rx_empty())
    }

    /// Is the RX queue full?
    fn rx_queue_is_full(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Rx {
            self.to_rx()?;
        }

        self.read_register::<FifoStatus>()
            .map(|(_, fifo_status)| fifo_status.rx_full())
    }

    /// Read the next received packet
    fn read(&mut self) -> Result<Payload, Self::Error> {
        if self.mode != Mode::Rx {
            self.to_rx()?;
        }

        let (_, payload_width) = self.send_command(&ReadRxPayloadWidth)?;
        let (_, payload) = self.send_command(&ReadRxPayload::new(payload_width as usize))?;
        Ok(payload)
    }
}

impl<'a, E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> Tx
    for NRF24L01<'a, E, CE, CSN, SPI>
{
    type Error = Error<SPIE>;

    fn tx_empty(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Tx {
            self.to_tx()?;
        }

        let (_, fifo_status) = self.read_register::<FifoStatus>()?;
        Ok(fifo_status.tx_empty())
    }

    fn tx_full(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Tx {
            self.to_tx()?;
        }

        let (_, fifo_status) = self.read_register::<FifoStatus>()?;
        Ok(fifo_status.tx_full())
    }

    fn can_send(&mut self) -> Result<bool, Self::Error> {
        if self.mode != Mode::Tx {
            self.to_tx()?;
        }

        let full = self.tx_full()?;
        Ok(!full)
    }

    fn send(&mut self, packet: &[u8]) -> Result<(), Self::Error> {
        if self.mode != Mode::Tx {
            self.to_tx()?;
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
            self.to_tx()?;
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
            self.to_tx()?;
        }

        let (_, observe_tx) = self.read_register()?;
        Ok(observe_tx)
    }
}

impl<'a, E: Debug, CE: OutputPin<Error = E>, CSN: OutputPin<Error = E>, SPI: SpiTransfer<u8, Error = SPIE>, SPIE: Debug> NRF24L01Configuration<'a>
    for NRF24L01<'a, E, CE, CSN, SPI>
{
    type Error = Error<SPIE>;

    fn flush_rx(&mut self) -> Result<(), Self::Error> {
        self.send_command(&FlushRx)?;
        Ok(())
    }

    fn flush_tx(&mut self) -> Result<(), Self::Error> {
        self.send_command(&FlushTx)?;
        Ok(())
    }

    fn set_rf_channel(&mut self, rf_channel: u8) -> Result<(), Self::Error> {
        assert!(rf_channel < 126);

        let mut register = RfCh(0);
        register.set_rf_ch(rf_channel);
        self.write_register(register)?;

        self.nrf_config.rf_channel = rf_channel;

        Ok(())
    }

    fn set_data_rate(&mut self, rate: DataRate) -> Result<(), Self::Error> {
        let power_level = &self.nrf_config.pa_level;

        let mut register = RfSetup(0);
        register.set_rf_pwr(match power_level {
            PALevel::PA0dBm => 3,
            PALevel::PA6dBm => 2,
            PALevel::PA12dBm => 1,
            PALevel::PA18dBm => 0,
        });

        let (dr_low, dr_high) = match rate {
            DataRate::R250Kbps => (true, false),
            DataRate::R1Mbps => (false, false),
            DataRate::R2Mbps => (false, true),
        };
        register.set_rf_dr_low(dr_low);
        register.set_rf_dr_high(dr_high);

        self.write_register(register)?;

        self.nrf_config.data_rate = rate;
        Ok(())
    }

    fn set_pa_level(&mut self, power: config::PALevel) -> Result<(), Self::Error> {
        let data_rate = &self.nrf_config.data_rate;

        let mut register = RfSetup(0);
        register.set_rf_pwr(match power {
            PALevel::PA0dBm => 3,
            PALevel::PA6dBm => 2,
            PALevel::PA12dBm => 1,
            PALevel::PA18dBm => 0,
        });

        let (dr_low, dr_high) = match data_rate {
            DataRate::R250Kbps => (true, false),
            DataRate::R1Mbps => (false, false),
            DataRate::R2Mbps => (false, true),
        };
        register.set_rf_dr_low(dr_low);
        register.set_rf_dr_high(dr_high);

        self.write_register(register)?;

        self.nrf_config.pa_level = power;
        Ok(())
    }

    fn set_crc_mode(&mut self, mode: CrcMode) -> Result<(), Self::Error> {
        match self.update_config(|config| {
            let (en_crc, crco) = match mode {
                CrcMode::Disabled => (false, false),
                CrcMode::OneByte => (true, false),
                CrcMode::TwoBytes => (true, true),
            };
            config.set_en_crc(en_crc);
            config.set_crco(crco);
        }) {
            Ok(_) => {
                self.nrf_config.crc_mode = mode;
                Ok(())
            },
            Err(err) => Err(err),
        }
    }

    fn set_interrupt_mask(&mut self, interrupt_mask: config::InterruptMask) -> Result<(), Self::Error> {
        match self.update_config(|config| {
            config.set_mask_rx_dr(interrupt_mask.data_ready_rx);
            config.set_mask_tx_ds(interrupt_mask.data_sent_tx);
            config.set_mask_max_rt(interrupt_mask.max_retramsits_tx);
        }) {
            Ok(_) => {
                self.nrf_config.interrupt_mask = interrupt_mask;
                Ok(())
            },
            Err(err) => Err(err),
        }
    }

    fn set_read_enabled_pipes(&mut self, read_enabled_pipes: &[bool; PIPES_COUNT]) -> Result<(), Self::Error> {
        match self.write_register(EnRxaddr::from_bools(read_enabled_pipes)) {
            Ok(_) => {
                self.nrf_config.read_enabled_pipes = *read_enabled_pipes;
                Ok(())
            },
            Err(err) => Err(err),
        }
    }

    fn set_rx_addr(&mut self, pipe_no: usize, addr: &'a [u8]) -> Result<(), Self::Error> {
        macro_rules! w {
            ( $($no: expr, $name: ident);+ ) => (
                match pipe_no {
                    $(
                        $no => {
                            use crate::registers::$name;
                            let register = $name::new(addr);
                            self.write_register(register)?;
                        }
                    )+
                        _ => panic!("No such pipe {}", pipe_no)
                }
            )
        }
        w!(0, RxAddrP0;
           1, RxAddrP1;
           2, RxAddrP2;
           3, RxAddrP3;
           4, RxAddrP4;
           5, RxAddrP5);

        self.nrf_config.rx_addr[pipe_no] = addr;
        Ok(())
    }

    fn set_tx_addr(&mut self, addr: &'a [u8]) -> Result<(), Self::Error> {
        let register = TxAddr::new(addr);
        self.write_register(register)?;
        self.nrf_config.tx_addr = addr;
        Ok(())
    }

    fn set_retransmit_config(&mut self, delay: u8, count: u8) -> Result<(), Self::Error> {
        let mut register = SetupRetr(0);
        register.set_ard(delay);
        register.set_arc(count);
        self.write_register(register)?;
        self.nrf_config.retransmit_config = RetransmitConfig { delay, count };
        Ok(())
    }

    fn set_auto_ack(&mut self, auto_ack_pipes: [bool; PIPES_COUNT]) -> Result<(), Self::Error> {
        let register = EnAa::from_bools(&auto_ack_pipes);
        self.write_register(register)?;
        self.nrf_config.auto_ack_pipes = auto_ack_pipes;
        Ok(())
    }

    fn set_address_width(&mut self, width: u8) -> Result<(), Self::Error> {
        let register = SetupAw(width - 2);
        self.write_register(register)?;
        self.nrf_config.address_width = width;
        Ok(())
    }

    fn set_pipes_payload_lengths(&mut self, lengths: [Option<u8>; PIPES_COUNT]) -> Result<(), Self::Error> {
        let mut bools = [true; PIPES_COUNT];
        for (i, len) in lengths.iter().enumerate() {
            bools[i] = len.is_none();
        }
        let dynpd = Dynpd::from_bools(&bools);
        if dynpd.0 != 0 {
            self.update_register::<Feature, _, _>(|feature| {
                feature.set_en_dpl(true);
            })?;
        }
        self.write_register(dynpd)?;

        // Set static payload lengths
        macro_rules! set_rx_pw {
            ($name: ident, $index: expr) => {{
                use crate::registers::$name;
                let length = lengths[$index].unwrap_or(0);
                let mut register = $name(0);
                register.set(length);
                self.write_register(register)?;
            }};
        }
        set_rx_pw!(RxPwP0, 0);
        set_rx_pw!(RxPwP1, 1);
        set_rx_pw!(RxPwP2, 2);
        set_rx_pw!(RxPwP3, 3);
        set_rx_pw!(RxPwP4, 4);
        set_rx_pw!(RxPwP5, 5);

        self.nrf_config.pipe_payload_lengths = lengths;

        Ok(())
    }

    fn set_nrf_configuration(&mut self, configuration: NRF24L01Config<'a>) -> Result<(), Self::Error> {
        if configuration.data_rate != self.nrf_config.data_rate {
            self.set_data_rate(configuration.data_rate)?;
        }

        if configuration.crc_mode != self.nrf_config.crc_mode {
            self.set_crc_mode(configuration.crc_mode)?;
        }

        if configuration.rf_channel != self.nrf_config.rf_channel {
            self.set_rf_channel(configuration.rf_channel)?;
        }

        if configuration.pa_level != self.nrf_config.pa_level {
            self.set_pa_level(configuration.pa_level)?;
        }

        if configuration.interrupt_mask != self.nrf_config.interrupt_mask {
            self.set_interrupt_mask(configuration.interrupt_mask)?;
        }

        if configuration.read_enabled_pipes != self.nrf_config.read_enabled_pipes {
            self.set_read_enabled_pipes(&configuration.read_enabled_pipes)?;
        }

        if configuration.rx_addr != self.nrf_config.rx_addr {
            for (pipe_no, addr) in configuration.rx_addr.iter().enumerate() {
                self.set_rx_addr(pipe_no, addr)?;
            }
        }

        if configuration.tx_addr != self.nrf_config.tx_addr {
            self.set_tx_addr(configuration.tx_addr)?;
        }

        if configuration.retransmit_config != self.nrf_config.retransmit_config {
            self.set_retransmit_config(configuration.retransmit_config.delay, configuration.retransmit_config.count)?;
        }

        if configuration.auto_ack_pipes != self.nrf_config.auto_ack_pipes {
            self.set_auto_ack(configuration.auto_ack_pipes)?;
        }

        if configuration.address_width != self.nrf_config.address_width {
            self.set_address_width(configuration.address_width)?;
        }

        if configuration.pipe_payload_lengths != self.nrf_config.pipe_payload_lengths {
            self.set_pipes_payload_lengths(configuration.pipe_payload_lengths)?;
        }

        Ok(())
    }

    fn get_data_rate(&self) -> DataRate {
        self.nrf_config.data_rate
    }

    fn get_crc_mode(&self) -> CrcMode {
        self.nrf_config.crc_mode
    }

    fn get_rf_channel(&self) -> u8 {
        self.nrf_config.rf_channel
    }

    fn get_pa_level(&self) -> PALevel {
        self.nrf_config.pa_level
    }

    fn get_interrupt_mask(&self) -> config::InterruptMask {
        self.nrf_config.interrupt_mask
    }

    fn get_read_enabled_pipes(&self) -> [bool; PIPES_COUNT] {
        self.nrf_config.read_enabled_pipes
    }

    fn get_rx_addr(&self) -> [&'a [u8]; PIPES_COUNT] {
        self.nrf_config.rx_addr
    }

    fn get_tx_addr(&self) -> &'a [u8] {
        self.nrf_config.tx_addr
    }

    fn get_retransmit_config(&self) -> RetransmitConfig {
        self.nrf_config.retransmit_config
    }

    fn get_auto_ack_pipes(&self) -> [bool; PIPES_COUNT] {
        self.nrf_config.auto_ack_pipes
    }

    fn get_address_width(&self) -> u8 {
        self.nrf_config.address_width
    }

    fn get_pipe_payload_lengths(&self) -> [Option<u8>; PIPES_COUNT] {
        self.nrf_config.pipe_payload_lengths
    }

    fn get_config(&self) -> NRF24L01Config {
        self.nrf_config
    }
}