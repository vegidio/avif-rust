//! Small helpers shared by the encoder and decoder for talking to libavif's C API.

use std::ffi::CStr;

use crate::sys;

/// Turn a non-OK [`avifResult`](sys::avifResult) into its human-readable message via
/// `avifResultToString`. Returns the empty string for `AVIF_RESULT_OK`.
pub(crate) fn result_message(result: sys::avifResult) -> String {
    // SAFETY: `avifResultToString` always returns a pointer to a static, NUL-terminated
    // C string for any input (including unknown codes).
    unsafe {
        let ptr = sys::avifResultToString(result);
        if ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

/// `true` when the result is `AVIF_RESULT_OK`.
pub(crate) fn is_ok(result: sys::avifResult) -> bool {
    result == sys::avifResult_AVIF_RESULT_OK
}
