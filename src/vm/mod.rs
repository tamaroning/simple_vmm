mod vcpu;

use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use kvm_ioctls::VmFd;
use linux_loader::loader::bootparam::{boot_params, CAN_USE_HEAP, KEEP_SEGMENTS};
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
            "[LOG] ioctl KVM_GET_VCPU_MMAP_SIZE = {:#x}",
            ctx.get_kvm().get_vcpu_mmap_size().unwrap()
        );

        // Create and init vCPU
        let vcpu = VCPU::new(ctx, &vm);

        Guest { vcpu, vm, mem }
    }

    pub fn load(&self, image_file: &str) {
        println!("[INFO] Start loading bzImage {}", image_file);

        let image = std::fs::read(image_file).unwrap();
        if image.len() > MEM_SIZE {
            eprintln!("Image file is too big");
            std::process::exit(1);
        }
        /*
        // TODO: unnecessary?
        unsafe {
            for (i, b) in image.iter().enumerate() {
                *(self.mem.wrapping_add(i)) = *b;
            }
        }
        */

        unsafe {
            let boot_params = self.mem.wrapping_add(0x10000) as *mut boot_params;
            let cmdline = self.mem.wrapping_add(0x20000);
            let kernel = self.mem.wrapping_add(0x100000);

            // Initialize boot parameters
            println!("[LOG] Copy boot parameters");
            *boot_params = *(image.as_ptr() as *const boot_params);
            let setup_sectors = (*boot_params).hdr.setup_sects as usize;
            let setup_size = (setup_sectors + 1) * 512;
            (*boot_params).hdr.vid_mode = 0xFFFF; // VGA
            (*boot_params).hdr.type_of_loader = 0xFF;
            (*boot_params).hdr.ramdisk_image = 0x0;
            (*boot_params).hdr.ramdisk_size = 0x0;
            (*boot_params).hdr.loadflags |= CAN_USE_HEAP as u8 | 0x01 | KEEP_SEGMENTS as u8;
            (*boot_params).hdr.heap_end_ptr = 0xFE00;
            (*boot_params).hdr.ext_loader_ver = 0x0;
            (*boot_params).hdr.cmd_line_ptr = 0x20000;

            // Clear command line
            println!("[LOG] Initalize kernel command line");
            for i in 0..((*boot_params).hdr.cmdline_size) {
                *(cmdline.wrapping_add(i as usize)) = 0;
            }
            // Append "console=ttyS0\0" to command line
            let tty_string = [
                b'c', b'o', b'n', b's', b'o', b'l', b'e', b'=', b't', b't', b'y', b'S', b'0', b'\0',
            ];
            for i in 0..tty_string.len() {
                *(cmdline.wrapping_add(i)) = tty_string[i];
            }

            // Copy kernel part
            println!("[LOG] Kernel size = {:#x}", image.len() - setup_size);
            for i in 0..(image.len() - setup_size) {
                *(kernel.wrapping_add(i)) =
                    *(image.as_ptr().wrapping_add(setup_size).wrapping_add(i));
            }
        }
    }

    fn get_mem_size(&self) -> usize {
        MEM_SIZE
    }

    pub fn run(&mut self) {
        println!("[INFO] Start running guest");
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
