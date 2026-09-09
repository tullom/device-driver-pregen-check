// This code was generated using device-driver `2.1.0` (),
// a tool distributed under MIT OR Apache-2.0 by Dion Dokter <dev@diondokter.nl>
//
// For more information about device-driver, visit the website: https://device-driver.com

/// Root block of the FixtureDevice driver
#[derive(Debug)]
pub struct FixtureDevice<I> {
    interface: I,
    #[doc(hidden)]
    #[allow(unused)]
    base_address: u8,
}
impl<I> FixtureDevice<I> {
    /// Create a new instance of the device
    pub const fn new(interface: I) -> Self {
        Self {
            interface,
            base_address: 0,
        }
    }
    /// Drop the driver instance and reclaim the interface
    pub fn free(self) -> I {
        self.interface
    }
    /// Register operation:
    /// - Address: `1`
    /// - Reset value: `0`
    pub fn configuration(
        &mut self,
    ) -> ::device_driver::RegisterOperation<'_, Self, Configuration, u8, ::device_driver::RW, ()>
    where
        I: ::device_driver::RegisterInterfaceBase<AddressType = u8>,
    {
        let address = self.base_address + 1;
        ::device_driver::RegisterOperation::new(self, address as u8, Configuration::default)
    }
}
impl<I> ::device_driver::Block for FixtureDevice<I> {
    type Interface = I;
    type RegisterAddressType = u8;
    type CommandAddressType = u8;
    type BufferAddressType = u8;
    type RegisterAddressMode = ();
    fn interface(&mut self) -> &mut Self::Interface {
        &mut self.interface
    }
}
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct Configuration {
    #[doc(hidden)]
    /// The internal bits
    bits: [u8; 1],
}
unsafe impl ::device_driver::Fieldset for Configuration {
    const METADATA: ::device_driver::FieldsetMetadata =
        ::device_driver::FieldsetMetadata::new().with_byte_order(::device_driver::ByteOrder::LE);
    const ZERO: Self = Self { bits: [0; 1] };
}
impl Configuration {
    /// `bit 0` - Read the `enabled` field.
    ///
    #[must_use]
    pub fn enabled(&self) -> bool {
        let start = 0;
        let end = 0;
        let raw = unsafe { ::device_driver::ops::load::<u8, ::device_driver::ops::LE>(&self.bits, start, end) };
        raw > 0
    }
    /// `2:1` - Read the `mode` field.
    ///
    #[must_use]
    pub fn mode(&self) -> u8 {
        let start = 1;
        let end = 2;
        let raw = unsafe { ::device_driver::ops::load::<u8, ::device_driver::ops::LE>(&self.bits, start, end) };
        raw
    }
    /// `bit 0` - Set the `enabled` field.
    ///
    pub fn set_enabled(&mut self, value: bool) {
        let start = 0;
        let end = 0;
        let raw = value as _;
        unsafe { ::device_driver::ops::store::<u8, ::device_driver::ops::LE>(raw, start, end, &mut self.bits) };
    }
    /// `2:1` - Set the `mode` field.
    ///
    pub fn set_mode(&mut self, value: u8) {
        let start = 1;
        let end = 2;
        let raw = value;
        unsafe { ::device_driver::ops::store::<u8, ::device_driver::ops::LE>(raw, start, end, &mut self.bits) };
    }
}
impl Default for Configuration {
    fn default() -> Self {
        <Self as ::device_driver::Fieldset>::ZERO
    }
}
impl From<[u8; 1]> for Configuration {
    fn from(bits: [u8; 1]) -> Self {
        Self { bits }
    }
}
impl From<Configuration> for [u8; 1] {
    fn from(val: Configuration) -> Self {
        val.bits
    }
}
impl core::fmt::Debug for Configuration {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        let mut d = f.debug_struct("Configuration");
        d.field("enabled", &self.enabled());
        d.field("mode", &self.mode());
        d.finish()
    }
}
impl core::ops::BitAnd for Configuration {
    type Output = Self;
    fn bitand(mut self, rhs: Self) -> Self::Output {
        self &= rhs;
        self
    }
}
impl core::ops::BitAndAssign for Configuration {
    fn bitand_assign(&mut self, rhs: Self) {
        for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
            *l &= *r;
        }
    }
}
impl core::ops::BitOr for Configuration {
    type Output = Self;
    fn bitor(mut self, rhs: Self) -> Self::Output {
        self |= rhs;
        self
    }
}
impl core::ops::BitOrAssign for Configuration {
    fn bitor_assign(&mut self, rhs: Self) {
        for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
            *l |= *r;
        }
    }
}
impl core::ops::BitXor for Configuration {
    type Output = Self;
    fn bitxor(mut self, rhs: Self) -> Self::Output {
        self ^= rhs;
        self
    }
}
impl core::ops::BitXorAssign for Configuration {
    fn bitxor_assign(&mut self, rhs: Self) {
        for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
            *l ^= *r;
        }
    }
}
impl core::ops::Not for Configuration {
    type Output = Self;
    fn not(mut self) -> Self::Output {
        for val in self.bits.iter_mut() {
            *val = !*val;
        }
        self
    }
}
