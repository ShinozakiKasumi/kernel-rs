//! GDT with kernel/user segments and a TSS for ring3 -> ring0 stack switches.
//!
//! Layout (matches the old C implementation):
//!   0x00 null
//!   0x08 kernel code (64-bit, DPL 0)
//!   0x10 kernel data          (DPL 0)
//!   0x18 user data            (DPL 3)
//!   0x20 user code (64-bit)   (DPL 3)
//!   0x28 TSS (16 bytes)

use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr::{addr_of, addr_of_mut};

pub const KERNEL_CS: u16 = 0x08;
pub const KERNEL_DS: u16 = 0x10;
pub const USER_DS: u16 = 0x18;
pub const USER_CS: u16 = 0x20;
pub const TSS_SEL: u16 = 0x28;

#[repr(C, packed)]
pub struct Tss {
    reserved0: u32,
    pub rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap_base: u16,
}

struct TssCell(UnsafeCell<Tss>);
unsafe impl Sync for TssCell {}

struct GdtCell(UnsafeCell<[u64; 7]>);
unsafe impl Sync for GdtCell {}

static GDT: GdtCell = GdtCell(UnsafeCell::new([0; 7]));
static TSS: TssCell = TssCell(UnsafeCell::new(Tss {
    reserved0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    iomap_base: 0,
}));

static mut BOOT_IRQ_STACK: [u8; 16384] = [0; 16384]; // 16KiB fallback for rsp0

#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u64,
}

pub fn set_kernel_stack(rsp0: u64) {
    unsafe { (*TSS.0.get()).rsp0 = rsp0 };
}

fn make_tss_desc_low(base: u64, limit: u32) -> u64 {
    (limit & 0xFFFF) as u64
        | ((base & 0xFF_FFFF) << 16)
        | (0x89u64 << 40) // present, type=64-bit TSS
        | (((limit >> 16) as u64 & 0xF) << 48)
        | ((base >> 24 & 0xFF) << 56)
}

pub fn init() {
    unsafe {
        let gdt = &mut *GDT.0.get();
        let tss = &mut *TSS.0.get();

        gdt[0] = 0x0000_0000_0000_0000; // null
        gdt[1] = 0x00AF_9A00_0000_FFFF; // kernel code
        gdt[2] = 0x00AF_9200_0000_FFFF; // kernel data
        gdt[3] = 0x00AF_F200_0000_FFFF; // user data
        gdt[4] = 0x00AF_FA00_0000_FFFF; // user code

        tss.rsp0 = addr_of_mut!(BOOT_IRQ_STACK) as u64 + BOOT_IRQ_STACK.len() as u64;
        tss.iomap_base = core::mem::size_of::<Tss>() as u16;
        let base = tss as *mut Tss as u64;
        let limit = (core::mem::size_of::<Tss>() - 1) as u32;
        gdt[5] = make_tss_desc_low(base, limit);
        gdt[6] = base >> 32;

        let gdtr = Gdtr {
            limit: (core::mem::size_of::<[u64; 7]>() - 1) as u16,
            base: gdt.as_ptr() as u64,
        };

        asm!(
            "lgdt [{gdtr}]",
            "mov ds, {ds:x}",
            "mov es, {ds:x}",
            "mov fs, {ds:x}",
            "mov gs, {ds:x}",
            "mov ss, {ds:x}",
            "push {cs}",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            gdtr = in(reg) &gdtr,
            ds = in(reg) KERNEL_DS as u64,
            cs = in(reg) KERNEL_CS as u64,
            out("rax") _,
        );

        crate::klog_info!("gdt: installed (cs={:#x} ds={:#x})", KERNEL_CS, KERNEL_DS);

        asm!("ltr {sel:x}", sel = in(reg) TSS_SEL, options(nomem, nostack));
        let rsp0 = addr_of!((*TSS.0.get()).rsp0).read_unaligned();
        crate::klog_info!("gdt: tss loaded, rsp0={:#x}", rsp0);
    }
}
