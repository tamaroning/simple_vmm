use std::io::Write;

use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use kvm_ioctls::Kvm;
use kvm_ioctls::VcpuExit;

fn main() {
    /*
    if std::env::args().len() != 2 {
        eprintln!("USAGE: {} <guest-image>", std::env::args().next().unwrap());
        std::process::exit(1);
    }
    */

    /*
    let asm_code: &[u8] = &[
        0xba, 0xf8, 0x03, /* mov $0x3f8, %dx */
        0x00, 0xd8, /* add %bl, %al */
        0x04, b'0', /* add $'0', %al */
        0xee, /* out %al, %dx */
        0xec, /* in %dx, %al */
        0xc6, 0x06, 0x00, 0x80, 0x00, /* movl $0, (0x8000); This generates a MMIO Write. */
        0x8a, 0x16, 0x00, 0x80, /* movl (0x8000), %dl; This generates a MMIO Read. */
        0xf4, /* hlt */
    ];
    */

    let asm_code: &[u8] = &[
        0xba, 0xf8, 0x03, /* mov $0x3f8, %dx */
        0x00, 0xd8, /* add %bl, %al */
        0x04, b'0', /* add $'0', %al */
        0xee, /* out %al, (%dx) */
        0xb0, b'\n', /* mov $'\n', %al */
        0xee,  /* out %al, (%dx) */
        0xf4,  /* hlt */
    ];

    let mem_size = 0x4000;
    let guest_addr = 0x1000;

    // ```
    // kvm_fd = open("/dev/kvm", O_RDWR)
    // ioctl(kvm_fd, KVM_CREATE_VM, 0));
    // ```
    let kvm = Kvm::new().unwrap();
    let vm = kvm.create_vm().unwrap();

    // この領域にゲストコードをロードする
    // ```
    // mmap(NULL, mem_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0)
    // ```
    let load_addr: *mut u8 = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            mem_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
            -1,
            0,
        ) as *mut u8
    };
    // ゲストコード領域(load_addr)にアセンブリを書き込む
    unsafe {
        let mut slice = std::slice::from_raw_parts_mut(load_addr, mem_size);
        slice.write(&asm_code).unwrap();
    }

    // ```
    // struct kvm_userspace_memory_region region;
    // region.xxx = ...;
    // ioctl(vm_fd, KVM_SET_USER_MEMORY_REGION, &region)
    // ```
    let slot = 0;
    let mem_region = kvm_userspace_memory_region {
        slot,
        guest_phys_addr: guest_addr, // ゲストの物理メモリ
        memory_size: mem_size as u64,
        userspace_addr: load_addr as u64, // ホスト側のメモリ領域
        flags: KVM_MEM_LOG_DIRTY_PAGES,
    };
    unsafe { vm.set_user_memory_region(mem_region).unwrap() };

    // Create one vCPU
    // ```
    // vcpu_fd = ioctl(vm_fd, KVM_CREATE_VCPU, 0))
    // ```
    let vcpu_fd = vm.create_vcpu(0).unwrap();

    // x86_64 specific registry setup
    // ``
    // struct kvm_sregs sregs;
    // sregs.xx = ...
    // ioctl(vcpu_fd, KVM_SET_SREGS, &sregs)
    // ```
    let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
    vcpu_sregs.cs.base = 0;
    vcpu_sregs.cs.selector = 0;
    vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    vcpu_regs.rip = guest_addr;
    vcpu_regs.rax = 2;
    vcpu_regs.rbx = 3;
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    // Run code on the vCPU
    loop {
        match vcpu_fd.run().expect("run failed") {
            VcpuExit::IoIn(addr, data) => {
                println!(
                    "Received an I/O in exit. Address: {:#x}. Data: {:#x}",
                    addr, data[0],
                );
            }
            VcpuExit::IoOut(addr, data) => {
                println!(
                    "Received an I/O out exit. Address: {:#x}. Data: {:#x}",
                    addr, data[0],
                );
            }
            VcpuExit::MmioRead(addr, _data) => {
                println!("Received an MMIO Read Request for the address {:#x}.", addr);
            }
            VcpuExit::MmioWrite(addr, _data) => {
                println!("Received an MMIO Write Request to the address {:#x}.", addr);
                // The code snippet dirties 1 page when it is loaded in memory
                let dirty_pages_bitmap = vm.get_dirty_log(slot, mem_size).unwrap();
                let dirty_pages = dirty_pages_bitmap
                    .into_iter()
                    .map(|page| page.count_ones())
                    .fold(0, |dirty_page_count, i| dirty_page_count + i);
                assert_eq!(dirty_pages, 1);
            }
            VcpuExit::Hlt => {
                break;
            }
            r => panic!("Unexpected exit reason: {:?}", r),
        }
    }

    unsafe {
        libc::munmap(load_addr as *mut libc::c_void, mem_size);
    }
}
