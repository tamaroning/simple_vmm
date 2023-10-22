mod vm;

use std::{
    cell::OnceCell,
    sync::{Mutex, OnceLock},
};

use kvm_ioctls::Kvm;
use vm::Guest;

struct Context {
    kvm: Kvm,
}

impl Context {
    pub fn new() -> Self {
        Context {
            kvm: Kvm::new().unwrap(),
        }
    }

    pub fn get_kvm(&self) -> &Kvm {
        &self.kvm
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("USAGE: {} <guest-image>", args[0]);
        std::process::exit(1);
    }

    let ctx = Context::new();

    let mut guest = Guest::new(&ctx, &args[1]);
    guest.run();
}
