# Shizuku: Limine-bootable x86_64 kernel, rewritten in Rust.
#
# Kernel: kernel-rs/ (cargo, custom x86_64-shizuku.json target, rust-lld).
# Userland: userspace-rs/ (Rust, custom user target) — see userspace-rs/.
# The legacy C tree (src/, kernel/, userspace/) is kept only until the
# Rust port has boot parity; the Makefile no longer builds it.

CARGO       := cargo
TARGET      := x86_64-shizuku.json
KERNEL_BIN  := target/x86_64-shizuku/release/kernel
KERNEL      := kernel.elf
HELLO_ELF   := userspace-rs/target/x86_64-shizuku-user/release/hello

.PHONY: all clean run iso qemu userland

all: $(KERNEL)

# --- kernel ----------------------------------------------------------------

$(KERNEL): userland $(shell find kernel-rs -name '*.rs')
	SHIZUKU_HELLO_ELF=$(abspath $(HELLO_ELF)) $(CARGO) build --release --manifest-path kernel-rs/Cargo.toml
	cp $(KERNEL_BIN) $@

# --- userland --------------------------------------------------------------

userland:
	cd userspace-rs && $(CARGO) build --release

initrd.tar: userland tools/mkinitrd.sh $(wildcard rootfs/etc/*)
	sh tools/mkinitrd.sh

# --- bootable image ----------------------------------------------------------

ISO_DIR := iso_root
ISO     := shizuku.iso
LIMINE  := limine

iso: $(KERNEL) initrd.tar
	mkdir -p $(ISO_DIR)/boot/limine
	cp $(KERNEL) $(ISO_DIR)/boot/
	cp initrd.tar $(ISO_DIR)/boot/
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
	rm -f initrd.tar $(KERNEL) $(ISO)
	rm -rf $(ISO_DIR)
	-$(CARGO) clean --manifest-path kernel-rs/Cargo.toml
	-$(CARGO) clean --manifest-path userspace-rs/Cargo.toml
