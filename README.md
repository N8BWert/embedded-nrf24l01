# embedded-nrf24l01

## Features

* Designed for use with the [embedded-hal] crate
* Safe and declarative register definitions
* Chip operation modes lifted to the type-level
* Lets you go straight into RX/TX with the default config

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


#### Default Configuration

```rust
let mut nrf24 = NRF24L01::new(ce, csn, spi).unwrap();
```

This will provide an instance of the NRF24L01 device in standby mode. To convert to different modes you can call `.to_rx()` to switch to Rx mode, `.to_tx()` for Tx mode, `.to_standby` for Standby Mode, and `.to_power_down()` to power down the device.  You can also just call a method belonging to a specific mode (i.e. `send()` for Tx mode) to switch to the given mode before conducting the given instruction.

#### Specified Configuration

```rust
let mut nrf24 = NRF24L01::new_with_config(ce, csn, spi, nrf_config).unwrap();
```

This will provide an instance of the NRF24L01 in standby mode (as above), but will also use the configuration provided to establish the nrf driver.

### Configuration

Before you start transmission, the device must be configured.

#### Configuration Options (+ Defaults)

- data_rate (`DataRate`): the rate to send data at (defaults to 250Kbps)
- crc_mode (`CrcMode`): the crc bit correction mode (defaults to Disabled)
- rf_channel (`u8`): the channel for this device to connect to (defaults to 0)
- pa_level (`PALevel`): the level of the device's power amplifier (defaults to -18dBm)
- interrupt_mask (`InterruptMask`): the interrupt mask (defaults to `000` or interrupts from data_ready_rx, data_set_tx, and max_transmits_tx are disabled)
- read_enabled_pipes (`[bool; 6]`): The pipes to read from (defaults to [`[false; 6]`])
- rx_addrs (`[&[u8]; 6]`): the addresses for each rx pipe to listen to (defaults to `[b"rx"; 6]`)
- tx_addr (`&[u8]`): the address to send data to (defaults to b"tx")
- retransmit_config (`RetransmitConfig`): the delay (ms) and number of times to resend packets when they are dropped (or not acknowledged) (defaults to {delay: 0, count: 0})
- auto_ack_pipes (`[bool; 6]`): the pipes configured to automatically acknowledge incoming messages
- address_width (`u8`): the width of the address to be used (between 3-5 bytes) (defaults to 3)
- pipe_payload_lengths (`[Option<u8>; 6]`): the length of the payload expected from each pipe (defaults to [None; 6] -- unknown/flexible payload length)

#### Setting single configurations

Getters and Setters are also provided on the nrf24l01 device to set and get any of the above configuration options.

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
