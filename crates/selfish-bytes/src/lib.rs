//! Bounds-checked fixed-width integer reads and writes at a byte offset.
//!
//! Every format crate reads and writes its fields through these functions. Each returns
//! `None` when the field does not fit, and the caller turns that into its own error, so an
//! offset past the end is always a value the caller sees and never a panic.
//!
//! ```
//! let mut bytes = [0_u8; 8];
//! selfish_bytes::write_le(&mut bytes, 4, 0x1234_u32).unwrap();
//! assert_eq!(selfish_bytes::read_le::<u32>(&bytes, 4), Some(0x1234));
//! assert_eq!(selfish_bytes::read_le::<u32>(&bytes, 6), None);
//! ```

#![forbid(unsafe_code)]

/// An unsigned integer these functions read and write.
pub trait Int: Copy + sealed::Sealed {
    /// The width in bytes.
    const WIDTH: usize;
    /// Decode from exactly [`Self::WIDTH`] little-endian bytes.
    fn from_le(raw: &[u8]) -> Option<Self>;
    /// Decode from exactly [`Self::WIDTH`] big-endian bytes.
    fn from_be(raw: &[u8]) -> Option<Self>;
    /// Encode into exactly [`Self::WIDTH`] bytes, little-endian.
    fn to_le(self, out: &mut [u8]);
    /// Encode into exactly [`Self::WIDTH`] bytes, big-endian.
    fn to_be(self, out: &mut [u8]);
}

mod sealed {
    pub trait Sealed {}
}

macro_rules! int {
    ($($t:ty),*) => {$(
        impl sealed::Sealed for $t {}
        impl Int for $t {
            const WIDTH: usize = core::mem::size_of::<$t>();
            fn from_le(raw: &[u8]) -> Option<Self> {
                raw.try_into().ok().map(<$t>::from_le_bytes)
            }
            fn from_be(raw: &[u8]) -> Option<Self> {
                raw.try_into().ok().map(<$t>::from_be_bytes)
            }
            fn to_le(self, out: &mut [u8]) {
                out.copy_from_slice(&self.to_le_bytes());
            }
            fn to_be(self, out: &mut [u8]) {
                out.copy_from_slice(&self.to_be_bytes());
            }
        }
    )*};
}

int!(u8, u16, u32, u64);

fn field<T: Int>(bytes: &[u8], at: usize) -> Option<&[u8]> {
    bytes.get(at..at.checked_add(T::WIDTH)?)
}

fn field_mut<T: Int>(bytes: &mut [u8], at: usize) -> Option<&mut [u8]> {
    bytes.get_mut(at..at.checked_add(T::WIDTH)?)
}

/// The little-endian integer at `at`, or `None` if it runs past the end.
#[must_use]
pub fn read_le<T: Int>(bytes: &[u8], at: usize) -> Option<T> {
    T::from_le(field::<T>(bytes, at)?)
}

/// The big-endian integer at `at`, or `None` if it runs past the end.
#[must_use]
pub fn read_be<T: Int>(bytes: &[u8], at: usize) -> Option<T> {
    T::from_be(field::<T>(bytes, at)?)
}

/// Write `value` little-endian at `at`. `None`, with nothing written, if it does not fit.
pub fn write_le<T: Int>(bytes: &mut [u8], at: usize, value: T) -> Option<()> {
    value.to_le(field_mut::<T>(bytes, at)?);
    Some(())
}

/// Write `value` big-endian at `at`. `None`, with nothing written, if it does not fit.
pub fn write_be<T: Int>(bytes: &mut [u8], at: usize, value: T) -> Option<()> {
    value.to_be(field_mut::<T>(bytes, at)?);
    Some(())
}

/// Copy `value` in at `at`. `None`, with nothing written, if it does not fit.
pub fn write_slice(bytes: &mut [u8], at: usize, value: &[u8]) -> Option<()> {
    bytes
        .get_mut(at..at.checked_add(value.len())?)?
        .copy_from_slice(value);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both byte orders read back what was written, at every width.
    #[test]
    fn round_trip() {
        let mut bytes = [0_u8; 16];
        assert_eq!(write_le(&mut bytes, 1, 0xBEEF_u16), Some(()));
        assert_eq!(read_le::<u16>(&bytes, 1), Some(0xBEEF));
        assert_eq!(write_be(&mut bytes, 4, 0x0102_0304_u32), Some(()));
        assert_eq!(bytes.get(4..8), Some(&[1, 2, 3, 4][..]));
        assert_eq!(read_be::<u32>(&bytes, 4), Some(0x0102_0304));
        assert_eq!(write_le(&mut bytes, 8, u64::MAX - 1), Some(()));
        assert_eq!(read_le::<u64>(&bytes, 8), Some(u64::MAX - 1));
    }

    /// A field that runs past the end, or whose end overflows, is `None` and writes nothing.
    #[test]
    fn out_of_range_is_none() {
        let mut bytes = [0_u8; 4];
        assert_eq!(read_le::<u32>(&bytes, 1), None);
        assert_eq!(read_be::<u16>(&bytes, usize::MAX), None);
        assert_eq!(write_le(&mut bytes, 2, u32::MAX), None);
        assert_eq!(write_be(&mut bytes, usize::MAX, 1_u64), None);
        assert_eq!(write_slice(&mut bytes, 3, &[1, 2]), None);
        assert_eq!(bytes, [0; 4]);
        assert_eq!(write_slice(&mut bytes, 2, &[1, 2]), Some(()));
        assert_eq!(bytes, [0, 0, 1, 2]);
    }
}
