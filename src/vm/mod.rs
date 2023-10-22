mod vcpu;

use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use kvm_ioctls::Kvm;
use kvm_ioctls::VmFd;

use self::vcpu::VCPU;

pub struct Guest {
    vm: VmFd,
    vcpu: VCPU,
    mem: *mut u8,
}

impl Guest {
    const MEM_SIZE: usize = 1 << 30;
    const PROGRAM_START: u64 = 0x0;

    pub fn new(kvm: &mut Kvm, image_file: &str) -> Self {
        let vm = kvm.create_vm().unwrap();

        let mem: *mut u8 = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                Self::MEM_SIZE,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
                -1,
                0,
            ) as *mut u8
        };

        let slot = 0;
        let mem_region = kvm_userspace_memory_region {
            slot,
            guest_phys_addr: Self::PROGRAM_START, // ゲストの物理メモリ
            memory_size: Self::MEM_SIZE as u64,
            userspace_addr: mem as u64, // ホスト側のメモリ領域
            flags: KVM_MEM_LOG_DIRTY_PAGES,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        unsafe {
            println!("Open image file: {}", image_file);
            let buf = std::fs::read(image_file).unwrap();
            if buf.len() > Self::MEM_SIZE {
                eprintln!("Image file is too big");
                std::process::exit(1);
            }
            dbg!(buf.len());
            for (i, b) in buf.iter().enumerate() {
                *(mem.wrapping_add(i)) = *b;
            }
        }

        println!(
            "ioctl KVM_GET_VCPU_MMAP_SIZE = {:#x}",
            kvm.get_vcpu_mmap_size().unwrap()
        );

        // Create one vCPU
        let vcpu_fd = vm.create_vcpu(0).unwrap();

        // x86_64 specific registry setup
        let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();

        vcpu_sregs.cs.base = 0;
        vcpu_sregs.cs.limit = u32::MAX;
        vcpu_sregs.cs.g = 0;

        vcpu_sregs.ds.base = 0;
        vcpu_sregs.ds.limit = u32::MAX;
        vcpu_sregs.ds.g = 1;

        vcpu_sregs.fs.base = 0;
        vcpu_sregs.fs.limit = u32::MAX;
        vcpu_sregs.fs.g = 1;

        vcpu_sregs.gs.base = 0;
        vcpu_sregs.gs.limit = u32::MAX;
        vcpu_sregs.gs.g = 1;

        vcpu_sregs.es.base = 0;
        vcpu_sregs.es.limit = u32::MAX;
        vcpu_sregs.es.g = 1;

        vcpu_sregs.ss.base = 0;
        vcpu_sregs.ss.limit = u32::MAX;
        vcpu_sregs.ss.g = 1;

        vcpu_sregs.cs.db = 1;
        vcpu_sregs.ss.db = 1;
        vcpu_sregs.cr0 |= 1; /* enable protected mode */

        vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

        let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
        vcpu_regs.rip = Self::PROGRAM_START;
        vcpu_regs.rflags = 2;
        vcpu_fd.set_regs(&vcpu_regs).unwrap();

        println!("RIP = {:#x}", vcpu_regs.rip);

        Guest {
            vm,
            vcpu: VCPU::new(vcpu_fd),
            mem,
        }
    }

    pub fn get_mem_size() -> usize {
        Self::MEM_SIZE
    }

    pub fn run(&mut self) {
        self.vcpu.run();
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.mem as *mut libc::c_void, Self::MEM_SIZE);
        }
    }
}
