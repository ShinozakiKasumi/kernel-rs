//! initrd: unpack a ustar archive delivered as a Limine module into tmpfs.

use crate::fs::vfs;
use crate::limine;

/* --- ustar ---------------------------------------------------------------- */

fn octal(s: &[u8]) -> u64 {
    let mut v: u64 = 0;
    for &b in s.iter().take_while(|&&b| b != 0) {
        if !(b'0'..=b'7').contains(&b) {
            break;
        }
        v = v * 8 + (b - b'0') as u64;
    }
    v
}

fn tar_unpack(base: &[u8]) -> i32 {
    let mut files = 0;
    let mut dirs = 0;
    let mut p = 0usize;
    while p + 512 <= base.len() {
        let blk = &base[p..];
        if blk[0] == 0 {
            break; // end-of-archive block
        }
        let name = core::str::from_utf8(&blk[..100])
            .unwrap_or("")
            .trim_end_matches('\0');
        let fsize = octal(&blk[124..136]) as usize;
        let ftype = blk[156];

        // build "/<name>" path, strip trailing '/' on dirs
        let mut path = [0u8; 128];
        path[0] = b'/';
        let n = name.len().min(126);
        path[1..1 + n].copy_from_slice(&name.as_bytes()[..n]);
        let mut plen = 1 + n;
        if plen > 1 && path[plen - 1] == b'/' {
            plen -= 1;
        }
        let path_str = match core::str::from_utf8(&path[..plen]) {
            Ok(s) => s,
            Err(_) => {
                p += 512 + (fsize + 511) & !511;
                continue;
            }
        };

        if ftype == b'5' {
            if vfs::mkdir(path_str) == 0 {
                dirs += 1;
            }
        } else if ftype == b'0' || ftype == 0 {
            // ensure parent dirs exist even if the tar lacks dir entries
            for q in 1..plen {
                if path[q] != b'/' {
                    continue;
                }
                let sub = match core::str::from_utf8(&path[..q]) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if vfs::path_type(sub) == 0 {
                    vfs::mkdir(sub);
                }
            }
            let data_end = (p + 512 + fsize).min(base.len());
            if vfs::create(path_str, &base[p + 512..data_end]) >= 0 {
                files += 1;
            } else {
                crate::klog_err!("initrd: failed {} ({} bytes)", path_str, fsize);
            }
        }
        p += 512 + (fsize + 511) & !511;
    }
    crate::klog_info!("initrd: unpacked {} files, {} dirs", files, dirs);
    files + dirs
}

/// Unpack the first module ("initrd") into tmpfs. Returns entries created.
pub fn unpack() -> i32 {
    let Some(r) = limine::module_response() else {
        crate::klog_warn!("initrd: no Limine module loaded");
        return 0;
    };
    if r.module_count == 0 {
        crate::klog_warn!("initrd: no Limine module loaded");
        return 0;
    }
    let m = unsafe { &*r.modules.read() };
    let path = unsafe { crate::util::cstr(m.path) };
    crate::klog_info!("initrd: {} ({} bytes)", path, m.size);
    let data = unsafe { core::slice::from_raw_parts(m.address, m.size as usize) };
    tar_unpack(data)
}
