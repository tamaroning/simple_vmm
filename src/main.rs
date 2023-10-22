use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use kvm_ioctls::Kvm;
use kvm_ioctls::VcpuExit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("USAGE: {} <guest-image>", args[0]);
        std::process::exit(1);
    }

    const MEM_SIZE: usize = 1 << 30;
    const PROGRAM_START: u64 = 0x0;

    let kvm = Kvm::new().unwrap();
    let vm = kvm.create_vm().unwrap();

    let load_addr: *mut u8 = unsafe {
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
        guest_phys_addr: PROGRAM_START, // ゲストの物理メモリ
        memory_size: MEM_SIZE as u64,
        userspace_addr: load_addr as u64, // ホスト側のメモリ領域
        flags: KVM_MEM_LOG_DIRTY_PAGES,
    };
    unsafe { vm.set_user_memory_region(mem_region).unwrap() };

    unsafe {
        println!("Open image file: {}", &args[1]);
        let buf = std::fs::read(&args[1]).unwrap();
        if (buf.len() > MEM_SIZE) {
            eprintln!("Image file is too big");
            std::process::exit(1);
        }
        dbg!(buf.len());
        for (i, b) in buf.iter().enumerate() {
            *(load_addr.wrapping_add(i)) = *b;
            dbg!((i, b));
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

    const CODE_START: u64 = 0;
    vcpu_sregs.cs.selector = CODE_START as u16;
    vcpu_sregs.cs.base = CODE_START * 16;
    vcpu_sregs.ss.selector = CODE_START as u16;
    vcpu_sregs.ss.base = CODE_START * 16;
    vcpu_sregs.ds.selector = CODE_START as u16;
    vcpu_sregs.ds.base = CODE_START * 16;
    vcpu_sregs.es.selector = CODE_START as u16;
    vcpu_sregs.es.base = CODE_START * 16;
    vcpu_sregs.fs.selector = CODE_START as u16;
    vcpu_sregs.fs.base = CODE_START * 16;
    vcpu_sregs.gs.selector = CODE_START as u16;

    /*
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

    //vcpu_sregs.cs.db = 1;
    //vcpu_sregs.ss.db = 1;
    //vcpu_sregs.cr0 |= 1; /* enable protected mode */
    */

    vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    vcpu_regs.rip = PROGRAM_START;
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    println!("RIP = {:#x}", vcpu_regs.rip);

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
            VcpuExit::MmioRead(addr, data) => {
                println!("Received an MMIO Read Request for the address {:#x}.", addr);
                //dbg!(data);
            }
            VcpuExit::MmioWrite(addr, data) => {
                println!("Received an MMIO Write Request to the address {:#x}.", addr);
                //dbg!(data);
                // The code snippet dirties 1 page when it is loaded in memory
                /*
                let dirty_pages_bitmap = vm.get_dirty_log(slot, mem_size).unwrap();
                let dirty_pages = dirty_pages_bitmap
                    .into_iter()
                    .map(|page| page.count_ones())
                    .fold(0, |dirty_page_count, i| dirty_page_count + i);
                assert_eq!(dirty_pages, 1);
                */
            }
            VcpuExit::Hlt => {
                println!("Halt");
                break;
            }
            VcpuExit::InternalError => {
                println!("Internal error");
                break;
            }
            VcpuExit::Shutdown => {
                println!("Shutdown");
                break;
            }
            r => panic!("Unexpected exit reason: {:?}", r),
        }
    }

    unsafe {
        libc::munmap(load_addr as *mut libc::c_void, MEM_SIZE);
    }
}
