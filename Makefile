# M0: Limine bootable x86_64 teaching kernel

CC      := x86_64-linux-gnu-gcc
LD      := x86_64-linux-gnu-ld
CFLAGS  := -Wall -Wextra -O2 -g \
           -std=c11 \
           -ffreestanding -fno-stack-protector -fno-pic -fno-pie \
           -nostdlib \
           -Iinclude -Isrc -Ikernel/gui \
           -mno-red-zone -mno-80387 -mno-mmx -mno-sse -mno-sse2 \
           -mcmodel=kernel
LDFLAGS := -nostdlib -static -z max-page-size=0x1000 -T linker.ld

SRC     := src/main.c src/uart.c src/kprintf.c src/fb.c \
           src/gdt.c src/idt.c src/pic.c \
           src/lib/string.c \
           src/mm/pmm.c src/mm/vmm.c \
           src/sched/sched.c \
           src/proc/elf.c src/proc/proc.c src/proc/syscall.c \
           src/fs/tmpfs.c src/kbd.c src/shell.c \
           kernel/gui/core/surface.c kernel/gui/core/mouse.c \
           kernel/gui/render/font.c kernel/gui/render/font8x16.c \
           kernel/gui/window/wm.c \
           src/isr.S
OBJ     := $(SRC:.c=.o)
OBJ     := $(OBJ:.S=.o)
OBJ     += userspace/hello_bin.o
KERNEL  := kernel.elf

.PHONY: all clean run

all: $(KERNEL)

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

%.o: %.S
	$(CC) $(CFLAGS) -c $< -o $@

# --- userland (ring-3 test programs) ---------------------------------------
UCFLAGS := -Wall -Wextra -O2 -ffreestanding -nostdlib -fno-pie -no-pie \
           -mno-sse -mno-red-zone -Iuserspace

userspace/hello.elf: userspace/hello.c userspace/link.ld
	$(CC) $(UCFLAGS) -T userspace/link.ld $< -o $@

userspace/hello_bin.o: userspace/hello.elf
	cd userspace && objcopy -I binary -O elf64-x86-64 -B i386:x86-64 \
	    hello.elf hello_bin.o

$(KERNEL): $(OBJ) linker.ld
	$(LD) $(LDFLAGS) $(OBJ) -o $@

# --- bootable image ----------------------------------------------------------
ISO_DIR := iso_root
ISO     := shizuku.iso
LIMINE  := limine

iso: $(KERNEL)
	mkdir -p $(ISO_DIR)/boot/limine
	cp $(KERNEL) $(ISO_DIR)/boot/
	cp limine.conf $(ISO_DIR)/boot/limine/
	cp $(LIMINE)/limine-bios.sys $(LIMINE)/limine-bios-cd.bin \
	   $(LIMINE)/limine-uefi-cd.bin $(ISO_DIR)/boot/limine/
	xorriso -as mkisofs -R -J \
	    -b boot/limine/limine-bios-cd.bin -no-emul-boot -boot-load-size 4 \
	    -boot-info-table --efi-boot boot/limine/limine-uefi-cd.bin \
	    -efi-boot-part --efi-boot-image $(ISO_DIR) -o $(ISO)
	$(LIMINE)/limine bios-install $(ISO)

qemu: iso
	qemu-system-x86_64 -M q35 -m 128M -cdrom $(ISO) -boot d \
	    -serial stdio -no-reboot

run: qemu

clean:
	rm -f src/*.o src/*/*.o userspace/*.o userspace/*.elf $(KERNEL)
