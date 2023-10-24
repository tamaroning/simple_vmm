use kvm_ioctls::VcpuExit;
use kvm_ioctls::VcpuFd;
use kvm_ioctls::VmFd;

use crate::Context;

pub struct Vcpu {
    vcpu_fd: VcpuFd,
}

impl Vcpu {
    pub fn new(ctx: &Context, vm: &VmFd) -> Self {
        println!("[LOG] Initialize vCPU registers");
        let vcpu_fd = vm.create_vcpu(0).unwrap();
        let vcpu = Vcpu { vcpu_fd };

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
        vcpu_sregs.cs.g = 1;

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
        vcpu_regs.rip = 0x10_0000;
        vcpu_regs.rsi = 0x1_0000;
        vcpu_regs.rflags = 2;
        self.vcpu_fd.set_regs(&vcpu_regs).unwrap();
    }

    // See 4.20 KVM_SET_CPUID, https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt
    fn init_cpu_id(&self, ctx: &Context) {
        print!("[LOG] Initialize CPUID entry");
        const NUM_ENTRY: usize = kvm_bindings::KVM_MAX_CPUID_ENTRIES;

        let mut cpuid = ctx.get_kvm().get_supported_cpuid(NUM_ENTRY).unwrap();
        for entry in cpuid.as_mut_slice() {
            // Define how to respond call to CPUID with EAX=<function>
            // See https://www.kernel.org/doc/html/v5.7/virt/kvm/cpuid.html
            if entry.function == /* KVM_CPUID_SIGNATURE */ 0x4000_0000 {
                entry.eax = /* KVM_CPUID_FEATURES */ 0x4000_0001;
                entry.ebx = 0x4b4d564b; // KVMK
                entry.ecx = 0x564b4d56; // VMKV
                entry.edx = 0x4d; // M
            }
        }
        self.vcpu_fd.set_cpuid2(&cpuid).unwrap();
    }

    pub fn run(&mut self) {
        loop {
            match self.vcpu_fd.run().expect("run failed") {
                VcpuExit::IoIn(_addr, _data) => {
                    // TODO: ignore for now
                }
                VcpuExit::IoOut(addr, data) => {
                    if addr == 0x3f8 {
                        print!("{}", data[0] as char);
                    } else {
                        // TODO: ignore for now
                    }
                }
                VcpuExit::MmioRead(addr, _data) => {
                    println!("Received an MMIO Read Request for the address {:#x}.", addr);
                }
                VcpuExit::MmioWrite(addr, _data) => {
                    println!("Received an MMIO Write Request to the address {:#x}.", addr);
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
