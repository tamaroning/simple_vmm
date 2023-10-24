mod vcpu;

use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use linux_loader::loader::bootparam::{boot_params, CAN_USE_HEAP, KEEP_SEGMENTS};
use vcpu::Vcpu;

use crate::Context;

const MEM_SIZE: usize = 1 << 30; // 1GB

pub struct Guest {
    #[allow(unused)]
    // FIXME: should remove this?
    vm: VmFd,
    vcpu: Vcpu,
    mem: *mut u8,
}

impl Guest {
    pub fn new(ctx: &Context) -> Self {
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

        Guest { vcpu, vm, mem }
    }

    pub fn load(&self, image_file: &str) {
        println!("[INFO] Start loading bzImage {}", image_file);

        let image = std::fs::read(image_file).unwrap();
        if image.len() > MEM_SIZE {
            eprintln!("Image file is too big");
            std::process::exit(1);
        }
        println!("[LOG] Image size = {:#x}", image.len());

        unsafe {
            let boot_params = self.mem.wrapping_add(0x1_0000) as *mut boot_params;
            let cmdline = self.mem.wrapping_add(0x2_0000);
            let kernel = self.mem.wrapping_add(0x10_0000);

            println!(
                "[LOG] boot_params = phys {:#x}",
                boot_params as usize - self.mem as usize
            );
            println!(
                "[LOG] cmdline = phys {:#x}",
                cmdline as usize - self.mem as usize
            );
            println!(
                "[LOG] kernel = phys {:#x}",
                kernel as usize - self.mem as usize
            );

            // Initialize boot parameters
            println!(
                "[LOG] Copy boot parameters to address {:#x}",
                boot_params as usize - self.mem as usize
            );
            *boot_params = *(image.as_ptr() as *const boot_params);
            let setup_sectors = (*boot_params).hdr.setup_sects as usize;
            println!("[LOG] setup_sectors = {:#x}", setup_sectors);
            let setup_size = (setup_sectors + 1) * 512;
            (*boot_params).hdr.vid_mode = 0xFFFF; // VGA
            (*boot_params).hdr.type_of_loader = 0xFF;
            (*boot_params).hdr.ramdisk_image = 0x0;
            (*boot_params).hdr.ramdisk_size = 0x0;
            (*boot_params).hdr.loadflags |= CAN_USE_HEAP as u8 | 0x01 | KEEP_SEGMENTS as u8;
            (*boot_params).hdr.heap_end_ptr = 0xFE00;
            (*boot_params).hdr.ext_loader_ver = 0x0;
            (*boot_params).hdr.cmd_line_ptr = 0x2_0000;

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
            const TTY_STRING: [u8; 14] = [
                b'c', b'o', b'n', b's', b'o', b'l', b'e', b'=', b't', b't', b'y', b'S', b'0', b'\0',
            ];
            for (i, c) in TTY_STRING.iter().enumerate() {
                *(cmdline.wrapping_add(i)) = *c;
            }

            // Copy kernel part
            let kernel_size = image.len() - setup_size;
            println!("[LOG] Setup size = {:#x}", setup_size);
            println!("[LOG] Kernel size = {:#x}", kernel_size);
            for i in 0..kernel_size {
                *(kernel.wrapping_add(i)) =
                    *(image.as_ptr().wrapping_add(setup_size).wrapping_add(i));
            }

            // overwrite kernel
            // FIXME: remove this
            /*
            let asm_code_ = [
                //0xba, 0xf8, 0x03, /* mov $0x3f8, %dx */
                0x00, 0xd8, /* add %bl, %al */
                //0x04, b'0',  /* add $'0', %al */
                0xee, /* out %al, (%dx) */
                0xb0, b'\n', /* mov $'\n', %al */
                0xee,  /* out %al, (%dx) */
                0xf4,  /* hlt */
            ];

            const ASM_CODE__: [u8; 12] = [
                0xc0, 0x31, 0x10, 0xe7, 0xeb, 0x40, 0x00, 0xfb, 0xf4, 0xf4, 0xf4, 0xf4,
            ];
            let asm_code = [
                0x0f, 0xa2, /* cpuid */
                //0xba, 0x01, 0x00, /* mov $0x1, %dx */
                0xee, /* out %al, (%dx) */
                0xf4, /* hlt */
            ];
            for i in 0..asm_code.len() {
                // *(kernel.wrapping_add(i)) = asm_code[i];
            }
            */
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
