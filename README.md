# shizuku kernel

教学级 x86-64 类 Unix 内核：Limine 引导、串口日志、PMM/VMM、抢占式内核线程(中断帧上下文切换)、用户态(int 0x80 syscall: write/exit)、ELF 加载、tmpfs、shell、软件合成 GUI(窗口管理/拖动/PS/2 鼠标/VGA 字体内核终端)。

## 构建与运行 (WSL2/Ubuntu)

```sh
sudo apt install make gcc-x86-64-linux-gnu binutils-x86-64-linux-gnu qemu-system-x86 xorriso socat
git clone https://github.com/limine-bootloader/limine.git --branch=v8.x-binary --depth=1 limine
make -C limine limine          # 生成 limine 部署工具

make                           # 内核 ELF
make iso                       # shizuku.iso(BIOS+UEFI 双模式)
make qemu                      # headless 串口
```

图形界面(经 WSLg 弹窗):

```sh
qemu-system-x86_64 -M q35 -m 128M -cdrom shizuku.iso -boot d -display gtk -serial stdio -no-reboot
```

## 目录

- `src/` — 引导入口、uart/kprintf、GDT/IDT/PIC/isr.S、fb、kbd、shell
- `src/mm/` — PMM(位图)与 VMM(四级页表)
- `src/sched/` — 时间片轮转调度器
- `src/proc/` — ELF 加载器、int 0x80 系统调用
- `src/fs/` — tmpfs
- `kernel/gui/` — surface/font(VGA 8x16)/PS/2 鼠标/窗口管理器与合成器
- `userspace/` — 用户态测试程序 + int 0x80 运行时(ulib.h)
- `external/` — 上游字体源文件
