//! Process layer: spawn a user process from an in-memory ELF image.
//!
//! The initial user stack follows the System V convention expected by
//! userspace ulib crt0:
//!
//!     USER_STACK_TOP ->
//!         [argc][argv0][argv1]...[argvN][NULL]
//!     ...below it the argument strings themselves.

pub mod elf;
pub mod elf_defs;

use crate::fs::vfs;
use crate::mm::pmm::{self, PAGE_SIZE};
use crate::mm::vmm::{self, PageTable, PTE_NX, PTE_RW, PTE_US};

pub const USER_STACK_TOP: u64 = 0x0000_7000_0000_0000;
pub const USER_STACK_PAGES: u64 = 4;
pub const USER_HEAP_BASE: u64 = 0x0000_6000_0000_0000;
pub const ARG_MAX: usize = 16;

/// Write bytes at user VA `dst` inside (inactive) space `space`.
/// Requires every touched page to be mapped already.
fn ustack_write(space: PageTable, mut dst: u64, src: &[u8]) -> bool {
    let mut p = 0usize;
    let mut len = src.len() as u64;
    while len > 0 {
        let pa = vmm::translate(space, dst);
        if pa == 0 {
            return false;
        }
        let mut chunk = PAGE_SIZE - (dst & (PAGE_SIZE - 1));
        if chunk > len {
            chunk = len;
        }
        // vmm::translate already includes the in-page offset.
        unsafe {
            core::ptr::copy_nonoverlapping(
                src.as_ptr().add(p),
                pmm::pa_to_va(pa),
                chunk as usize,
            );
        }
        dst += chunk;
        p += chunk as usize;
        len -= chunk;
    }
    true
}

/// Spawn from an in-memory ELF image. Returns tid or -1.
pub fn spawn_elf(name: &str, elf_data: &[u8], argv: &[&str]) -> i32 {
    let space = vmm::new_space();
    if space == 0 {
        crate::klog_err!("proc: no address space");
        return -1;
    }

    let entry = elf::elf_load(space, elf_data);
    if entry == 0 {
        vmm::destroy_space(space);
        return -1;
    }

    // user stack right below USER_STACK_TOP
    for i in 0..USER_STACK_PAGES {
        let va = USER_STACK_TOP - (USER_STACK_PAGES - i) * PAGE_SIZE;
        let pa = pmm::alloc_page();
        if pa == 0 || !vmm::map(space, va, pa, PTE_US | PTE_RW | PTE_NX) {
            crate::klog_err!("proc: stack alloc failed");
            vmm::destroy_space(space);
            return -1;
        }
        unsafe {
            core::ptr::write_bytes(pmm::pa_to_va(pa), 0, PAGE_SIZE as usize);
        }
    }

    // Build argv on the user stack (strings first, top-down).
    let mut sp = USER_STACK_TOP;
    let mut user_argv = [0u64; ARG_MAX + 1];
    let argc = argv.len().min(ARG_MAX);
    for i in (0..argc).rev() {
        let bytes = argv[i].as_bytes();
        sp -= bytes.len() as u64 + 1;
        if !ustack_write(space, sp, bytes)
            || !ustack_write(space, sp + bytes.len() as u64, &[0])
        {
            crate::klog_err!("proc: argv copy failed");
            vmm::destroy_space(space);
            return -1;
        }
        user_argv[i] = sp;
    }

    // Entry stack frame: [argc][argv0..argvN][NULL] with rsp 16-aligned.
    let block = ((argc as u64 + 2) * 8 + 15) & !15;
    sp = (sp & !15) - block; // entry rsp
    let mut frame = [0u64; 18 + 2]; // argc + up to 17 argv + pad
    frame[0] = argc as u64;
    frame[1..1 + argc + 1].copy_from_slice(&user_argv[..argc + 1]);
    // Only the live prefix [argc][argv0..argvN][NULL] is written; the C
    // version sized the copy to (argc+2)*8 — the rest of the array is
    // uninitialised stack space and must not spill past USER_STACK_TOP.
    let live = &frame[..(argc + 2)];
    if !ustack_write(space, sp, bytemuck_slice(live)) {
        vmm::destroy_space(space);
        return -1;
    }

    let tid = crate::sched::thread_create_user(name, entry, sp, space);
    if tid < 0 {
        vmm::destroy_space(space);
        return -1;
    }

    if let Some(t) = crate::sched::thread_at(tid as usize) {
        t.brk = USER_HEAP_BASE;
    }
    tid
}

fn bytemuck_slice(v: &[u64]) -> &[u8] {
    unsafe { core::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 8) }
}

/// Spawn a user process by filesystem path. Returns tid or -1.
pub fn spawn_path(path: &str, argv: &[&str]) -> i32 {
    let id = vfs::lookup(path);
    if id < 0 || vfs::node_type(id) != vfs::VN_FILE {
        return -1;
    }
    let Some(data) = vfs::node_data(id) else {
        return -1;
    };
    if data.is_empty() {
        return -1;
    }

    // short name for the thread table
    let base = path.rsplit('/').next().unwrap_or(path);
    let base = if base.is_empty() { path } else { base };
    spawn_elf(base, data, argv)
}
