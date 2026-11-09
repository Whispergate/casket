//! Uniform access to a connected device, whatever the protocol underneath.

use crate::core::autodetect::DeviceInfo;
use crate::error::Result;

/// A device that is already connected.
///
/// Construction stays per protocol because the arguments differ: an S7 rack and slot is not a Modbus
/// unit id. What is unified is what you can do once you hold one, so generic code does not need a match
/// arm per protocol.
pub trait DeviceClient {
    /// Protocol level facts already learned from the device.
    ///
    /// Borrowed, not rebuilt per call, so a caller can hold the reference across a few statements
    /// without a clone. The record is what the connect path learned: a vendor that has not yet read
    /// identity through the session reports what it did learn, not an empty record.
    fn device_info(&self) -> &DeviceInfo;

    /// Release the socket. Idempotent: calling it twice is not an error.
    fn close(&mut self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // No type in the tree stores a unified `DeviceInfo` yet: each vendor keeps its own device
    // struct and converts on demand through `IntoDeviceInfo`. So the bound is proven here against
    // a private stub, as the queue entry allows, and the vendor adoption is left to Task 12.
    struct StubDevice {
        info: DeviceInfo,
        closed: bool,
    }

    impl StubDevice {
        fn new(ip: &str) -> Self {
            Self {
                info: DeviceInfo {
                    vendor: "stub".into(),
                    ip: ip.into(),
                    fields: HashMap::new(),
                },
                closed: false,
            }
        }
    }

    impl DeviceClient for StubDevice {
        fn device_info(&self) -> &DeviceInfo {
            &self.info
        }

        fn close(&mut self) -> Result<()> {
            self.closed = true;
            Ok(())
        }
    }

    // The whole point of the trait: a bound, used as one.
    fn bound<T: DeviceClient>() {}

    #[test]
    fn the_bound_compiles_for_an_implementor() {
        bound::<StubDevice>();
    }

    #[test]
    fn device_info_is_the_stored_record() {
        bound::<StubDevice>();
        let device = StubDevice::new("192.0.2.10");
        assert_eq!(device.device_info().vendor, "stub");
        assert_eq!(device.device_info().ip, "192.0.2.10");
        assert!(device.device_info().fields.is_empty());
    }

    #[test]
    fn close_is_idempotent() {
        bound::<StubDevice>();
        let mut device = StubDevice::new("192.0.2.10");
        assert!(device.close().is_ok());
        assert!(device.close().is_ok());
        assert!(device.closed);
    }
}
