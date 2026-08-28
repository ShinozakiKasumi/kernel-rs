# shizuku kernel

教学级 x86-64 类 Unix 内核，**全量 Rust 实现**(`#![no_std]`, nightly + `build-std`):Limine 引导、串口日志、PMM/VMM、抢占式内核线程(中断帧上下文切换)、完整用户态(int 0x80 ABI，22 个系统调用)、ELF 加载、目录树 tmpfs、initrd(ustar)、用户态 shell + coreutils、软件合成 GUI。

## 基础软件生态

- **ABI**: `int 0x80`，rax=调用号，rdi/rsi/rdx 传参，错误返回负值；编号定义于 `kernel-rs/src/syscall.rs`，用户态封装在 `userspace-rs/src/ulib.rs`。
- **ulib**(`userspace-rs/src/ulib.rs`)：Rust `#![no_std]` crt0(`global_asm` 入口 + argv 解包)、syscall 封装、string/printf/malloc(sbrk)。
- **/bin**(17 个): echo ls cat pwd mkdir touch rm cp mv sleep clear ps mem uname hexdump true + `/bin/sh`、`/sbin/init`，全部 Rust(`userspace-rs/src/bin/`)。
- **命令约定**: 每个命令支持 `--help`、出错向 fd2 打印 `cmd: arg: reason`、返回非 0。
- **rootfs**: `rootfs/`(etc/motd/version/hostname) + `tools/mkinitrd.sh` → `initrd.tar`(ustar) → Limine module → 内核启动时解包进 tmpfs 并启动 `/sbin/init` → `/bin/sh`，sh 退出后 init 自动重启。
- 串口即控制台：QEMU `-serial stdio` 的输入经 UART RX 注入键盘队列，可直接敲命令(亦可写脚本驱动)。

## 构建与运行 (WSL2/Ubuntu)

```sh
sudo apt install make qemu-system-x86 xorriso
rustup toolchain install nightly --profile minimal --component rust-src
git clone https://github.com/limine-bootloader/limine.git --branch=v8.x-binary --depth=1 limine
make -C limine limine          # 生成 limine 部署工具

make iso                       # kernel-rs + userspace-rs + initrd → shizuku.iso
make qemu                      # headless 串口(可直接交互)
```

验收冒烟（启动后在串口输入）:

```sh
ls /bin            # 17 个命令 + sh
cat /etc/version   # shizuku 0.2-userspace x86_64
ps; mem; uname     # 进程表 / pmm 统计 / uname
cd /tmp && touch a && cp a b && mv b c && rm a c && ls   # 空目录
exit               # init 重启 shell
```

图形界面(经 WSLg 弹窗):

```sh
qemu-system-x86_64 -M q35 -m 128M -cdrom shizuku.iso -boot d -display gtk -serial stdio -no-reboot
```

## 目录

- `kernel-rs/src/` — 引导入口(main.rs)、uart/log、GDT/IDT/PIC/中断 stub(asm.rs)、fb、kbd、shell
- `kernel-rs/src/mm/` — PMM(位图)与 VMM(四级页表)
- `kernel-rs/src/sched.rs` — 时间片轮转调度器
- `kernel-rs/src/proc/` — ELF 加载器、进程创建(argv/brk),`syscall.rs` int 0x80 系统调用层
- `kernel-rs/src/fs/` — 目录树 tmpfs、initrd(ustar) 解包、vfs
- `kernel-rs/src/gui/` — surface/font(VGA 8x16)/PS/2 鼠标/窗口管理器与合成器
- `userspace-rs/` — ulib 运行时 + bin/ 17 个 coreutils + sh + sbin/init
- `rootfs/` + `tools/mkinitrd.sh` — 初始文件系统与打包
- `x86_64-shizuku.json` / `rust-toolchain.toml` / `.cargo/config.toml` — 自定义 target、nightly + build-std 工具链

历史 C 实现已移除；git 历史中可查。
