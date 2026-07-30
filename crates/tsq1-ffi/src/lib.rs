//! C-compatible dynamic library for TSQ1 conversion.

pub use tsq1_core::ffi::{Tsq1Buffer, Tsq1Status};

/// Convert SMF bytes into TSQ1 format, allocating a new buffer for the result.
///
/// The caller is responsible for freeing the resulting buffer with
/// [`tsq1_buffer_free`].
///
/// # Safety
///
/// `midi_ptr` must point to `midi_len` readable bytes, and `out` must point to
/// writable storage for one [`Tsq1Buffer`]. The returned allocation must be
/// released exactly once with [`tsq1_buffer_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsq1_mid_to_tsq(
    midi_ptr: *const u8,
    midi_len: usize,
    out: *mut Tsq1Buffer,
) -> Tsq1Status {
    unsafe { tsq1_core::ffi::tsq1_mid_to_tsq(midi_ptr, midi_len, out) }
}

/// Release a buffer produced by [`tsq1_mid_to_tsq`].
///
/// # Safety
///
/// `buf` must be either the untouched value returned by
/// [`tsq1_mid_to_tsq`] or a buffer whose pointer is null. A non-null buffer
/// must be passed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsq1_buffer_free(buf: Tsq1Buffer) {
    unsafe {
        tsq1_core::ffi::tsq1_buffer_free(buf);
    }
}
