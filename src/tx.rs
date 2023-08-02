use crate::registers::ObserveTx;

/// Represents **TX Mode** and the associated **TX Settling** and
/// **Standby-II** states
///
/// # Timing
///
/// The datasheet states the follwing:
///
/// > It is important to never keep the nRF24L01 in TX mode for more than 4ms at a time.
///
/// No effects have been observed when exceeding this limit. The
/// warranty could get void.
pub trait Tx {
    /// Error from performing TX Operations (Most commonly this will only be spi errors)
    type Error;

    /// Is TX FIFO empty?
    fn tx_empty(&mut self) -> Result<bool, Self::Error>;

    /// Is TX FIFO full?
    fn tx_full(&mut self) -> Result<bool, Self::Error>;

    /// Does the TX FIFO have space?
    fn can_send(&mut self) -> Result<bool, Self::Error>;

    /// Send asynchronously
    fn send(&mut self, packet: &[u8]) -> Result<(), Self::Error>;

    /// Poll completion of one or multiple send operations and check whether transmission was
    /// successful.
    ///
    /// This function behaves like `wait_empty()`, except that it returns whether sending was
    /// successful and that it provides an asynchronous interface.
    fn poll_send(&mut self) -> nb::Result<bool, Self::Error>;

    /// Clears tx interrupts and disables the device (sets ce to false)
    fn clear_tx_interrupts_and_ce(&mut self) -> nb::Result<(), Self::Error>;

    /// Wait until TX FIFO is empty
    ///
    /// If any packet cannot be delivered and the maximum amount of retries is
    /// reached, the TX FIFO is flushed and all other packets in the FIFO are
    /// lost.
    fn wait_empty(&mut self) -> Result<(), Self::Error>;

    /// Read the `OBSERVE_TX` register
    fn observe(&mut self) -> Result<ObserveTx, Self::Error>;
}

