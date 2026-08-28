//! Port I/O and CPU control primitives.

use core::arch::asm;

#[inline(always)]
pub fn outb(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags)) }
}

#[inline(always)]
pub fn inb(port: u16) -> u8 {
    let ret: u8;
    unsafe { asm!("in al, dx", out("al") ret, in("dx") port, options(nomem, nostack, preserves_flags)) }
    ret
}

#[inline(always)]
pub fn outl(port: u16, val: u32) {
    unsafe { asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack, preserves_flags)) }
}

#[inline(always)]
pub fn inl(port: u16) -> u32 {
    let ret: u32;
    unsafe { asm!("in eax, dx", out("eax") ret, in("dx") port, options(nomem, nostack, preserves_flags)) }
    ret
}

#[inline(always)]
pub fn io_wait() {
    unsafe { asm!("out dx, al", in("dx") 0x80u16, in("al") 0u8, options(nomem, nostack, preserves_flags)) }
}

#[inline(always)]
pub fn sti() {
    unsafe { asm!("sti", options(nomem, nostack)) }
}

#[inline(always)]
pub fn cli() {
    unsafe { asm!("cli", options(nomem, nostack)) }
}

#[inline(always)]
pub fn hlt() {
    unsafe { asm!("hlt", options(nomem, nostack)) }
}

#[inline(always)]
pub fn interrupts_enabled() -> bool {
    let flags: u64;
    unsafe { asm!("pushfq", "pop {}", out(reg) flags, options(nomem)) }
    flags & 0x200 != 0
}

/// Run `f` with interrupts disabled, restoring the previous IF state.
pub fn without_interrupts<R>(f: impl FnOnce() -> R) -> R {
    let were = interrupts_enabled();
    cli();
    let r = f();
    if were {
        sti();
    }
    r
}

#[inline(always)]
pub fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack, preserves_flags)
        )
    }
    ((hi as u64) << 32) | lo as u64
}

#[inline(always)]
pub fn wrmsr(msr: u32, val: u64) {
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") val as u32,
            in("edx") (val >> 32) as u32,
            options(nomem, nostack, preserves_flags)
        )
    }
}

#[inline(always)]
pub fn read_cr3() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr3", out(reg) v, options(nomem, nostack, preserves_flags)) }
    v
}

#[inline(always)]
pub fn write_cr3(v: u64) {
    unsafe { asm!("mov cr3, {}", in(reg) v, options(nostack)) }
}

#[inline(always)]
pub fn invlpg(addr: u64) {
    unsafe { asm!("invlpg [{}]", in(reg) addr, options(nostack)) }
}
