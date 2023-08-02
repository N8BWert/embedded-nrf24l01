# embedded-nrf24l01

## Features

* Designed for use with the [embedded-hal] crate
* Safe and declarative register definitions
* Chip operation modes lifted to the type-level
* Lets you go straight into RX/TX with the default config

### Still missing

* Auto-ack support

## Reference datasheets

* [nRF24L01+](https://www.sparkfun.com/datasheets/Components/SMD/nRF24L01Pluss_Preliminary_Product_Specification_v1_0.pdf)

## Usage

### Parameters

Get the `*-hal` crate for your micro-controller unit. Figure out how
to get to the peripherals implementing these [embedded-hal] traits:

* `embedded_hal::blocking::spi::Transfer` for the SPI peripheral

  We provide a `mod setup` with a few constants for SPI.
 
* `embedded_hal::digital::OutputPin` for the **CE** pin

* `embedded_hal::digital::OutputPin` for the **CSN** pin

  (Although that one belongs to the SPI, we found it much more
  reliable to implement in software.)

### Constructor

```rust
let mut nrf24 = NRF24L01::new(ce, csn, spi).unwrap();
```

This will provide an instance of the NRF24L01 device in standby mode. To convert to different modes you can call `.to_rx()` to switch to Rx mode, `.to_tx()` for Tx mode, `.to_standby` for Standby Mode, and `.to_power_down()` to power down the device.  You can also just call a method belonging to a specific mode (i.e. `send()` for Tx mode) to switch to the given mode before conducting the given instruction.


### Configuration

Before you start transmission, the device must be configured. Example:

```rust
nrf24.set_channel(8)?;
nrf24.set_auto_retransmit(0, 0)?;
nrf24.set_rf(&nrf24::DataRate::R2Mbps, 3)?;
nrf24.set_pipes_rx_enable(&[true, false, false, false, false, false])?;
nrf24.set_auto_ack(&[false; 6])?;
nrf24.set_crc(&nrf24::CrcMode::Disabled)?;
nrf24.set_tx_addr(&b"fnord"[..])?;
```

### `RXMode`

Use `rx.can_read()` to poll (returning the pipe number), then
`rx.read()` to receive payload.

### `TXMode`

Use `send(packet: &[u8])` to enqueue a packet.

Use `can_send()` to prevent sending on a full queue, and
`wait_empty()` to flush.


[embedded-hal]: https://crates.io/crates/embedded-hal

## Note

I forked this from [astro/embedded-nrf24l01](https://github.com/astro/embedded-nrf24l01/tree/master) because I wanted to use the nRF24L01 radio with RTIC and consuming the radio instance did not play well with RTIC's shared and local memory systems.  Therefore, this repo is literally Astro's work just repackaged to play a bit better with RTIC.
