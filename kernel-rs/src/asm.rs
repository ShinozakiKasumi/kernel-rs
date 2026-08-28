//! Interrupt entry stubs, thread trampoline, and user-mode entry helpers.
//!
//! Ported 1:1 from src/isr.S. Each stub normalises the stack to
//! `[vector][err][rip][cs][rflags]([rsp][ss])`, then `isr_common` pushes the
//! general registers (matching `IrqFrame`) and calls `isr_handler`.

use core::arch::global_asm;

// Syscall/vector stubs. `intel_syntax` flips operands to Intel order.
global_asm!(
    r#"
// Exceptions that push a hardware error code:
// 8 #DF, 10 #TS, 11 #NP, 12 #SS, 13 #GP, 14 #PF, 17 #AC, 21 #CP, 29 #VC,
// 30 #SX.
.macro ISR_NOERR n
.globl isr_stub_\n
isr_stub_\n:
    push 0
    push \n
    jmp isr_common
.endm

.macro ISR_ERR n
.globl isr_stub_\n
isr_stub_\n:
    push \n
    jmp isr_common
.endm

ISR_NOERR 0
ISR_NOERR 1
ISR_NOERR 2
ISR_NOERR 3
ISR_NOERR 4
ISR_NOERR 5
ISR_NOERR 6
ISR_NOERR 7
ISR_ERR   8
ISR_NOERR 9
ISR_ERR   10
ISR_ERR   11
ISR_ERR   12
ISR_ERR   13
ISR_ERR   14
ISR_NOERR 15
ISR_NOERR 16
ISR_ERR   17
ISR_NOERR 18
ISR_NOERR 19
ISR_NOERR 20
ISR_ERR   21
ISR_NOERR 22
ISR_NOERR 23
ISR_NOERR 24
ISR_NOERR 25
ISR_NOERR 26
ISR_NOERR 27
ISR_NOERR 28
ISR_ERR   29
ISR_ERR   30
ISR_NOERR 31
// vectors 32..47: PIC IRQ lines (32 timer, 33 keyboard), no error code
ISR_NOERR 32
ISR_NOERR 33
ISR_NOERR 34
ISR_NOERR 35
ISR_NOERR 36
ISR_NOERR 37
ISR_NOERR 38
ISR_NOERR 39
ISR_NOERR 40
ISR_NOERR 41
ISR_NOERR 42
ISR_NOERR 43
ISR_NOERR 44
ISR_NOERR 45
ISR_NOERR 46
ISR_NOERR 47
// 0x80: system call gate
ISR_NOERR 128
.purgem ISR_NOERR
.purgem ISR_ERR

// Common tail. Push order must match struct IrqFrame in idt.rs.
isr_common:
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    mov rdi, rsp
    call isr_handler

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax

    add rsp, 16        // discard vector + err
    iretq

// Stub table: 48 consecutive 8-byte addresses, one per vector 0..47.
.section .rodata
.globl isr_stub_table
.align 8
isr_stub_table:
    .quad isr_stub_0,  isr_stub_1,  isr_stub_2,  isr_stub_3
    .quad isr_stub_4,  isr_stub_5,  isr_stub_6,  isr_stub_7
    .quad isr_stub_8,  isr_stub_9,  isr_stub_10, isr_stub_11
    .quad isr_stub_12, isr_stub_13, isr_stub_14, isr_stub_15
    .quad isr_stub_16, isr_stub_17, isr_stub_18, isr_stub_19
    .quad isr_stub_20, isr_stub_21, isr_stub_22, isr_stub_23
    .quad isr_stub_24, isr_stub_25, isr_stub_26, isr_stub_27
    .quad isr_stub_28, isr_stub_29, isr_stub_30, isr_stub_31
    .quad isr_stub_32, isr_stub_33, isr_stub_34, isr_stub_35
    .quad isr_stub_36, isr_stub_37, isr_stub_38, isr_stub_39
    .quad isr_stub_40, isr_stub_41, isr_stub_42, isr_stub_43
    .quad isr_stub_44, isr_stub_45, isr_stub_46, isr_stub_47
.text

// Continuation for the far-return CS reload in gdt::init.
.globl gdt_reload_cs
gdt_reload_cs:
    ret

// First-run entry for a freshly created kernel thread.
// Expects rbx = entry function, rdi = arg (set by sched's fabricated frame).
.globl kthread_trampoline
kthread_trampoline:
    call rbx
    call sched_thread_exit   // fn returned: exit via scheduler
.Lhang:
    hlt
    jmp .Lhang
"#
);
