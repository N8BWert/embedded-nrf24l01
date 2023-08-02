/// Mode for the nRF24L01+ Device
#[derive(PartialEq)]
pub enum Mode {
    /// Standby Mode (Standby-I Mode in the Datasheet).  This mode is meant
    /// to ensure low power usage when there is no data being sent or received.
    Standby,
    /// Power Down Mode.  This mode is used for the nRF24L01 to consumer minimal
    /// current.  The register values of the device are maintained, but switching
    /// to Standby, Rx, and Tx takes significantly longer
    PowerDown,
    /// Sets the Device as a Receiver.  In this mode the nRF24L01 device will
    /// actively receive packets and insert them into the RX FIFOs slots
    Rx,
    /// Sets the Device as a Transmitter.  In this mode the nRF24L01 device will
    /// actively send packets from the TX FIFO register.  Please Stay in Standby or Read when
    /// there is nothing being sent because the manufacturer says bad things happen when
    /// in tx for a long time (not sure why, we haven't seen any issues with it but who knows)
    Tx,
}

/// Change the nRF24L01+ Device between different modes defined in the datasheet
pub trait ChangeModes {
    /// Error for changing the device types (most likely a SPI error)
    type Error;

    /// Converts the device into Standby-I as defined in the datasheet
    fn to_standby(&mut self) -> Result<(), Self::Error>;

    /// Converts the device into Power Down mode as defined in the Mode enum and in the
    /// datasheet
    fn to_power_down(&mut self) -> Result<(), Self::Error>;

    /// Converts the device into RX mode as defined in the Mode enum and
    /// the datasheet
    fn to_rx(&mut self) -> Result<(), Self::Error>;

    /// Converts the device into TX mode (and Standby-II if no data is in
    /// TX FIFO) as defined in the Mode enum and the datasheet
    fn to_tx(&mut self) -> Result<(), Self::Error>;
}
