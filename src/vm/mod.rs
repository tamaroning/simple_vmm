mod vcpu;

use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use kvm_ioctls::VmFd;

use vcpu::VCPU;

use crate::Context;

const MEM_SIZE: usize = 1 << 30;

pub struct Guest {
    vm: VmFd,
    vcpu: VCPU,
    mem: *mut u8,
}

impl Guest {
    pub fn new(ctx: &Context) -> Self {
        let vm = ctx.get_kvm().create_vm().unwrap();

        // TODO: ioctl KVM_SET_TSS_ADDR
        // TODO: KVM_SET_IDENTITY_MAP_ADDR
        // TODO: KVM_CREATE_IRQCHIP
        // TODO: KVM_CREATE_PIT2

        let mem: *mut u8 = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                MEM_SIZE,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
                -1,
                0,
            ) as *mut u8
        };

        let slot = 0;
        let mem_region = kvm_userspace_memory_region {
            slot,
            guest_phys_addr: 0,
            memory_size: MEM_SIZE as u64,
            userspace_addr: mem as u64,
            flags: KVM_MEM_LOG_DIRTY_PAGES,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        println!(
            "ioctl KVM_GET_VCPU_MMAP_SIZE = {:#x}",
            ctx.get_kvm().get_vcpu_mmap_size().unwrap()
        );

        // Create and init vCPU
        let vcpu = VCPU::new(ctx, &vm);

        Guest { vcpu, vm, mem }
    }

    pub fn load(&self, image_file: &str) {
        unsafe {
            println!("Open image file: {}", image_file);
            let buf = std::fs::read(image_file).unwrap();
            if buf.len() > MEM_SIZE {
                eprintln!("Image file is too big");
                std::process::exit(1);
            }
            dbg!(buf.len());
            for (i, b) in buf.iter().enumerate() {
                *(self.mem.wrapping_add(i)) = *b;
            }
        }
    }

    fn get_mem_size(&self) -> usize {
        MEM_SIZE
    }

    pub fn run(&mut self) {
        self.vcpu.run();
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.mem as *mut libc::c_void, MEM_SIZE);
        }
    }
}
