mod vcpu;

use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use linux_loader::loader::bootparam::{boot_params, CAN_USE_HEAP, KEEP_SEGMENTS};
use vcpu::Vcpu;

use crate::Context;

pub struct Guest {
    #[allow(unused)]
    // FIXME: should remove this?
    vm: VmFd,
    vcpu: Vcpu,
    mem: *mut u8,
    mem_size: usize,
}

impl Guest {
    pub fn new(ctx: &Context) -> Self {
        const MEM_SIZE: usize = 1 << 30; // 1GB

        let vm = ctx.get_kvm().create_vm().unwrap();

        vm.set_tss_address(0xffffd000).unwrap();
        vm.set_identity_map_address(0xffffc000).unwrap();
        vm.create_irq_chip().unwrap();
        vm.create_pit2(kvm_bindings::kvm_pit_config {
            flags: 0,
            pad: [0; 15],
        })
        .unwrap();

        let mem: *mut u8 = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                MEM_SIZE,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            ) as *mut u8
        };

        let mem_region = kvm_userspace_memory_region {
            slot: 0,
            flags: 0,
            guest_phys_addr: 0,
            memory_size: MEM_SIZE as u64,
            userspace_addr: mem as u64,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        println!(
            "[LOG] ioctl KVM_GET_VCPU_MMAP_SIZE = {:#x}",
            ctx.get_kvm().get_vcpu_mmap_size().unwrap()
        );

        // Create and init vCPU
        let vcpu = Vcpu::new(ctx, &vm);

        Guest {
            vcpu,
            vm,
            mem,
            mem_size: MEM_SIZE,
        }
    }

    pub fn load(&self, image_file: &str) {
        println!("[LOG] Start loading Linux Kernel {}", image_file);

        let image = std::fs::read(image_file).unwrap();
        if image.len() > self.get_mem_size() {
            eprintln!("[ERROR] Image file is too big");
            std::process::exit(1);
        } else if image.len() < 10 * 1024 {
            // Kernel should be at least 10KB
            eprintln!("[ERROR] Image file is too small");
            std::process::exit(1);
        }
        println!("[LOG] Image size = {:#x}", image.len());

        unsafe {
            let boot_params = self.mem.wrapping_add(0x10000) as *mut boot_params;
            let cmdline = self.mem.wrapping_add(0x20000);
            let kernel = self.mem.wrapping_add(0x100000);

            // Initialize boot parameters
            *boot_params = *(image.as_ptr() as *const boot_params);
            let setup_sectors = (*boot_params).hdr.setup_sects as usize;
            let setup_size = (setup_sectors + 1) * 512;
            (*boot_params).hdr.vid_mode = 0xFFFF; // VGA
            (*boot_params).hdr.type_of_loader = 0xFF;
            // TODO: initrd
            // https://github.com/bobuhiro11/gokvm/blob/12d6157f506c057a742096e101f9274089203ebf/kvm/kvm.go
            (*boot_params).hdr.ramdisk_image = 0x0;
            (*boot_params).hdr.ramdisk_size = 0x0;
            (*boot_params).hdr.loadflags |= CAN_USE_HEAP as u8 | 0x01 | KEEP_SEGMENTS as u8;
            (*boot_params).hdr.heap_end_ptr = 0xFE00;
            (*boot_params).hdr.ext_loader_ver = 0x0;
            (*boot_params).hdr.cmd_line_ptr = 0x20000;

            // Clear command line
            let cmdline_size = (*boot_params).hdr.cmdline_size;
            println!(
                "[LOG] Initalize kernel command line. size = {:#x}",
                cmdline_size
            );
            for i in 0..((*boot_params).hdr.cmdline_size) {
                *(cmdline.wrapping_add(i as usize)) = 0;
            }
            // Append "console=ttyS0\0" to command line
            const KERNEL_PARAMS: &str="console=ttyS0\0";
            assert!(KERNEL_PARAMS.is_ascii());
            for (i, c) in KERNEL_PARAMS.chars().map(|c| c  as u8).enumerate() {
                *(cmdline.wrapping_add(i)) = c;
            }

            // Copy kernel part
            let kernel_size = image.len() - setup_size;
            for i in 0..kernel_size {
                *(kernel.wrapping_add(i)) =
                    *(image.as_ptr().wrapping_add(setup_size).wrapping_add(i));
            }
        }
    }

    fn get_mem_size(&self) -> usize {
        self.mem_size
    }

    pub fn run(&mut self) {
        println!("[INFO] Start running guest");
        self.vcpu.run();
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.mem as *mut libc::c_void, self.get_mem_size());
        }
    }
}
