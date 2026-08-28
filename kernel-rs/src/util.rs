//! Small freestanding helpers (C-string reads, byte-string compare...).

/// Read a NUL-terminated C string as &str (lossy on invalid UTF-8 is not
/// available without alloc; callers pass valid ASCII paths).
///
/// # Safety
/// `p` must point at a valid NUL-terminated string.
pub unsafe fn cstr(p: *const core::ffi::c_char) -> &'static str {
    let mut len = 0usize;
    while !p.is_null() && *p.add(len) != 0 {
        len += 1;
    }
    let slice = core::slice::from_raw_parts(p as *const u8, len);
    core::str::from_utf8(slice).unwrap_or("<??>")
}

/// Fill a fixed buffer with the bytes of `s` (truncated), NUL padded.
pub fn buf_copy(dst: &mut [u8], s: &str) {
    let n = s.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&s.as_bytes()[..n]);
    dst[n..].fill(0);
}
