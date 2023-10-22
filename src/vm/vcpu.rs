use kvm_ioctls::VcpuExit;
use kvm_ioctls::VcpuFd;
use kvm_ioctls::VmFd;

use crate::Context;

const INITIAL_RIP: u64 = 0; //0x10_0000; // TODO: replace
const INITIAL_RSI: u64 = 0x1_0000;

pub struct VCPU {
    vcpu_fd: VcpuFd,
}

impl VCPU {
    pub fn new(ctx: &Context, vm: &VmFd) -> Self {
        let vcpu_fd = vm.create_vcpu(0).unwrap();
        let vcpu = VCPU { vcpu_fd };

        // x86_64 specific registry setup
        vcpu.init_sregs();
        vcpu.init_regs();
        vcpu.init_cpu_id(ctx);
        vcpu
    }

    fn init_sregs(&self) {
        let mut vcpu_sregs = self.vcpu_fd.get_sregs().unwrap();

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

        self.vcpu_fd.set_sregs(&vcpu_sregs).unwrap();
    }

    fn init_regs(&self) {
        let mut vcpu_regs = self.vcpu_fd.get_regs().unwrap();
        vcpu_regs.rip = INITIAL_RIP;
        vcpu_regs.rsi = INITIAL_RSI;
        vcpu_regs.rflags = 2;
        self.vcpu_fd.set_regs(&vcpu_regs).unwrap();
    }

    // See 4.20 KVM_SET_CPUID, https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt
    fn init_cpu_id(&self, ctx: &Context) {
        const NUM_ENTRY: usize = kvm_bindings::KVM_MAX_CPUID_ENTRIES;

        let mut fam = ctx.get_kvm().get_supported_cpuid(NUM_ENTRY).unwrap();

        for entry in fam.as_mut_slice() {
            if entry.function == /* KVM_CPUID_SIGNATURE */ 0x40000000 {
                entry.eax = /* KVM_CPUID_FEATURES */0x40000001;
                entry.ebx = 0x4b4d564b; // KVMK
                entry.ecx = 0x564b4d56; // VMKV
                entry.edx = 0x4d; // M
            }
        }
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
