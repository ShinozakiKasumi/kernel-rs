//! shizuku kernel — entry point and boot orchestration (Rust rewrite).
//!
//! Boot order mirrors the original C kernel:
//!   uart -> gdt -> idt -> pmm -> vmm -> sched -> vfs/initrd -> userspace
//!   -> int3 selftest -> fb/mouse/gui -> serial console pump.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

#[allow(non_snake_case)]
mod asm;
mod fb;
mod fs;
mod gdt;
mod gui;
mod idt;
mod io;
mod kbd;
mod limine;
mod log;
mod mm;
mod pic;
mod proc;
mod sched;
mod shell;
mod syscall;
mod uart;
mod util;
mod vterm;

use core::panic::PanicInfo;

/// Embedded fallback userspace program (replaces the C
/// `_binary_hello_elf_start/end` symbols).
static HELLO_ELF: &[u8] = include_bytes!(env!("SHIZUKU_HELLO_ELF"));

#[no_mangle]
extern "C" fn kernel_main() -> ! {
    uart::init();
    klog_info!("boot: limine ok, serial up");

    gdt::init(); // our own GDT (kernel cs/ds + user segments + TSS)
    idt::init(); // exceptions + IRQ stubs, PIC remapped

    mm::pmm::init(); // physical page allocator from Limine memmap
    mm::pmm::selftest();
    mm::vmm::init(); // own kernel CR3, HHDM kept, identity dropped
    mm::vmm::selftest();

    sched::init(); // round-robin threads, boot context = tid 0
    unsafe {
        idt::SCHED_ENABLED = true;
    }
    idt::enable(); // sti

    sched::selftest(); // two ticker threads must interleave

    fs::vfs::init();
    let unpacked = fs::initrd::unpack();

    if unpacked > 0 && proc::spawn_path("/sbin/init", &[]) >= 0 {
        klog_info!("userspace: /sbin/init spawned");
    } else {
        // fallback: no initrd -> kernel shell + embedded hello
        klog_warn!("no initrd: falling back to kernel shell");
        fs::vfs::create("/hello.elf", HELLO_ELF);
        sched::kthread_create("shell", shell::shell_main, core::ptr::null_mut());
        proc::spawn_elf("hello", HELLO_ELF, &[]);
    }

    // breakpoint vector 3 must round-trip through the asm stubs
    unsafe {
        core::arch::asm!("int3");
    }
    klog_info!("int3 test: survived");

    if fb::init() == 0 {
        gui::mouse::init();
        gui::wm::init(); // windows + compositor + terminal mirror
    } else {
        klog_warn!("fb: unavailable, GUI disabled");
    }

    klog_info!("timer/keyboard interrupts live — type to test kbd");

    // Serial console input: inject RX bytes into the console terminal
    // (vterm 0 once the GUI is up, legacy keyboard buffer otherwise).
    loop {
        if let Some(c) = uart::getc() {
            if vterm::console_up() {
                vterm::push(0, c);
            } else {
                kbd::push_char(c);
            }
        }
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // raw write: the logger may itself be compromised
    uart::write_bytes(b"\n*** KERNEL PANIC ***\n");
    if let Some(loc) = info.location() {
        klog_err!("panic at {}:{}: {}", loc.file(), loc.line(), info.message());
    } else {
        klog_err!("panic: {}", info.message());
    }
    loop {
        unsafe {
            core::arch::asm!("cli; hlt");
        }
    }
}
