use kvm_ioctls::VcpuExit;
use kvm_ioctls::VcpuFd;

pub struct VCPU {
    vcpu_fd: VcpuFd,
}

impl VCPU {
    pub fn new(fd: VcpuFd) -> Self {
        VCPU { vcpu_fd: fd }
    }

    pub fn run(&mut self) {
        loop {
            match self.vcpu_fd.run().expect("run failed") {
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
                    //dbg!(data);
                }
                VcpuExit::MmioWrite(addr, _data) => {
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
    }
}
