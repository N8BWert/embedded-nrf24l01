use crate::PIPES_COUNT;

/// Supported air data rates.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum DataRate {
    /// 250 Kbps
    R250Kbps,
    /// 1 Mbps
    R1Mbps,
    /// 2 Mbps
    R2Mbps,
}

impl Default for DataRate {
    fn default() -> DataRate {
        DataRate::R1Mbps
    }
}

/// Supported CRC modes
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CrcMode {
    /// Disable all CRC generation/checking
    Disabled,
    /// One byte checksum
    OneByte,
    /// Two bytes checksum
    TwoBytes,
}

/// The Power Amplifier Control Level for the nRF24L01 power amplifier (negative)
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PALevel {
    /// 0 dBm
    PA0dBm,
    /// -6 dBm
    PA6dBm,
    /// -12 dBm
    PA12dBm,
    /// -18 dBm
    PA18dBm,
}

/// Interrupt Masks grouped together into a single struct
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct InterruptMask {
    /// Trip Interrupt when data is available to be read
    pub data_ready_rx: bool,
    /// Trip Interrupt when data has been sent
    pub data_sent_tx: bool,
    /// Trip interrupt when the maximum retries has been hit for a transmission
    pub max_retramsits_tx: bool,
}

/// Retransmit Configuration grouped together into a single struct
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct RetransmitConfig {
    /// The number of miliseconds to wait before retrying transmission
    pub delay: u8,
    /// The number of retransmissions to attempt
    pub count: u8,
}

/// A software struct organizing the configuration of the NRF24L01.  I might end up
/// changing this because it is technically possible for the hardware to change and
/// not allert the software
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct NRF24L01Config<'a> {
    /// The rate to send data at
    pub data_rate: DataRate,
    /// The crc bit correction mode
    pub crc_mode: CrcMode,
    /// The RF channel for this device to listen on
    pub rf_channel: u8,
    /// The power amplifier level
    pub pa_level: PALevel,
    /// The interrupt mask
    pub interrupt_mask: InterruptMask,
    /// The pipes that are to be read from
    pub read_enabled_pipes: [bool; PIPES_COUNT],
    /// The addresses to read from (per pipe)
    pub rx_addr: [&'a [u8]; PIPES_COUNT],
    /// The address to transmit to
    pub tx_addr: &'a [u8],
    /// At what delay and how many times should data be retransmitted
    pub retransmit_config: RetransmitConfig,
    /// Should we sent an auto acknowledgement to data received at these pipes
    pub auto_ack_pipes: [bool; PIPES_COUNT],
    /// the address width for enhanced shockburst (3-5 bytes)
    pub address_width: u8,
    /// The length of data to expect from each pipe
    pub pipe_payload_lengths: [Option<u8>; PIPES_COUNT],
}

impl<'a> NRF24L01Config<'a> {
    /// Creates a new instance of NRF24L01Config with given parameters
    pub fn new(
        data_rate: DataRate,
        crc_mode: CrcMode,
        rf_channel: u8,
        pa_level: PALevel,
        interrupt_mask: InterruptMask,
        read_enabled_pipes: [bool; PIPES_COUNT],
        rx_addr: [&'a [u8]; PIPES_COUNT],
        tx_addr: &'a [u8],
        retransmit_config: RetransmitConfig,
        auto_ack_pipes: [bool; PIPES_COUNT],
        address_width: u8,
        pipe_payload_lengths: [Option<u8>; PIPES_COUNT],
    ) -> Self {
        Self {
            data_rate,
            crc_mode,
            rf_channel,
            pa_level,
            interrupt_mask,
            read_enabled_pipes,
            rx_addr,
            tx_addr,
            retransmit_config,
            auto_ack_pipes,
            address_width,
            pipe_payload_lengths,
        }
    }
}

impl<'a> Default for NRF24L01Config<'a> {
    fn default() -> Self {
        Self {
            data_rate: DataRate::R1Mbps,
            crc_mode: CrcMode::Disabled,
            rf_channel: 0u8,
            pa_level: PALevel::PA18dBm,
            interrupt_mask: InterruptMask { data_ready_rx: false, data_sent_tx: false, max_retramsits_tx: false },
            read_enabled_pipes: [false; PIPES_COUNT],
            rx_addr: [b"rx"; PIPES_COUNT],
            tx_addr: b"tx",
            retransmit_config: RetransmitConfig { delay: 0u8, count: 0u8 },
            auto_ack_pipes: [false; PIPES_COUNT],
            address_width: 0u8,
            pipe_payload_lengths: [None; PIPES_COUNT],
        }
    }
}

/// Trait for a device to implement to modify the various aspects of the NRF24L01 Configuration
pub trait NRF24L01Configuration<'a> {
    /// The error type to return on unsuccessful operation (most likely SPI error)
    type Error;

    /// Flush RX queue
    ///
    /// Discards all received packets that have not yet been [read](struct.RxMode.html#method.read) from the RX FIFO
    fn flush_rx(&mut self) -> Result<(), Self::Error>;

    /// Flush TX queue, discarding any unsent packets
    fn flush_tx(&mut self) -> Result<(), Self::Error>;

    /// Set the RF channel to transmit and receive from
    fn set_rf_channel(&mut self, rf_channel: u8) -> Result<(), Self::Error>;

    /// Sets the data rate to transmit data
    fn set_data_rate(&mut self, rate: DataRate) -> Result<(), Self::Error>;

    /// Sets the power amplifier level
    fn set_pa_level(&mut self, power: PALevel) -> Result<(), Self::Error>;

    /// Sets the bit correction mode
    fn set_crc_mode(&mut self, mode: CrcMode) -> Result<(), Self::Error>;

    /// Sets the interrupt mask
    fn set_interrupt_mask(&mut self, interrupt_mask: InterruptMask) -> Result<(), Self::Error>;

    /// Sets the pipes that are read-enabled
    fn set_read_enabled_pipes(&mut self, read_enabled_pipes: &[bool; PIPES_COUNT]) -> Result<(), Self::Error>;

    /// Sets the read address of a specific pipe
    fn set_rx_addr(&mut self, pipe_no: usize, addr: &'a [u8]) -> Result<(), Self::Error>;

    /// Sets the address to send data to
    fn set_tx_addr(&mut self, addr: &'a [u8]) -> Result<(), Self::Error>;

    /// Sets the delay and number of retransmissions for failed transmissions
    fn set_retransmit_config(&mut self, delay: u8, count: u8) -> Result<(), Self::Error>;

    /// Sets which pipes should automatically send an ack message
    fn set_auto_ack(&mut self, auto_ack_pipes: [bool; PIPES_COUNT]) -> Result<(), Self::Error>;

    /// Sets the width of the address for outgoing and incoming transmissions (between 3 and 5 bytes)
    fn set_address_width(&mut self, width: u8) -> Result<(), Self::Error>;

    /// Sets the expected payload length for each of the rx pipes (defaults to None = dynamic payload length)
    fn set_pipes_payload_lengths(&mut self, lengths: [Option<u8>; PIPES_COUNT]) -> Result<(), Self::Error>;

    /// Sets all of the fields of the nrf configuration
    fn set_nrf_configuration(&mut self, configuration: NRF24L01Config<'a>) -> Result<(), Self::Error>;

    /// Gets the data transmission rate
    fn get_data_rate(&self) -> DataRate;

    /// Gets the bit correction mode
    fn get_crc_mode(&self) -> CrcMode;

    /// Gets the radio channel
    fn get_rf_channel(&self) -> u8;

    /// Gets the radio's power amplification level
    fn get_pa_level(&self) -> PALevel;

    /// Gets the interrupt mask for the radio
    fn get_interrupt_mask(&self) -> InterruptMask;

    /// Gets an array of pipes with whether/not they are read enabled
    fn get_read_enabled_pipes(&self) -> [bool; PIPES_COUNT];

    /// Gets the rx addresses of each pipe
    fn get_rx_addr(&self) -> [&'a [u8]; PIPES_COUNT];

    /// Gets the tx address
    fn get_tx_addr(&self) -> &'a [u8];

    /// Get configuration for retransmits
    fn get_retransmit_config(&self) -> RetransmitConfig;
    
    /// Get a list of pipes with whether or not they will auto acknowledge
    fn get_auto_ack_pipes(&self) -> [bool; PIPES_COUNT];

    /// Gets the address with (between 3-5 bytes)
    fn get_address_width(&self) -> u8;

    /// Gets the payload length of each pipe
    fn get_pipe_payload_lengths(&self) -> [Option<u8>; PIPES_COUNT];

    /// Gets the full NRF24L01 configuraiton
    fn get_config(&self) -> NRF24L01Config;
}