use crate::payload::Payload;

/// Represents **RX Mode**
pub trait Rx {
    /// Error from read states (most commonly SPI errors as device modes are switched whenever
    /// this trait is used)
    type Error;

    /// Checks whether there is any incoming data to read.
    /// 
    /// If there is data, we'll get the pipe number of the data
    fn can_read(&mut self) -> Result<Option<u8>, Self::Error>;

    /// Is an in-band RF signal detected?
    ///
    /// The internal carrier detect signal must be high for 40μs
    /// (NRF24L01+) or 128μs (NRF24L01) before the carrier detect
    /// register is set. Note that changing from standby to receive
    /// mode also takes 130μs.
    fn has_carrier(&mut self) -> Result<bool, Self::Error>;

    /// Is the RX queue empty?
    fn rx_queue_empty(&mut self) -> Result<bool, Self::Error>;

    /// Is the RX queue full?
    fn rx_queue_is_full(&mut self) -> Result<bool, Self::Error>;

    /// Read the next received packet
    fn read(&mut self) -> Result<Payload, Self::Error>;
}
