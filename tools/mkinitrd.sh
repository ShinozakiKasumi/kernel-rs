#!/bin/sh
# Pack rootfs/ + built userland into initrd.tar (ustar, consumed by the kernel).
set -e
cd "$(dirname "$0")/.."

rm -rf rootfs/bin rootfs/sbin
mkdir -p rootfs/bin rootfs/sbin rootfs/dev rootfs/proc rootfs/tmp rootfs/home

for f in userspace-rs/target/x86_64-shizuku-user/release/*; do
    [ -f "$f" ] || continue
    name=$(basename "$f")
    case "$name" in
        init) cp "$f" rootfs/sbin/init ;;
        *.d|*.rlib|*.rmeta) continue ;;
        *)    cp "$f" rootfs/bin/ ;;
    esac
done

# GNU tar, deterministic ustar
tar --format=ustar --owner=0 --group=0 --numeric-owner \
    -cf initrd.tar -C rootfs .
echo "initrd.tar: $(tar -tf initrd.tar | wc -l) entries"
