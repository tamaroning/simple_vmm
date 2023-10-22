mod vm;

use kvm_ioctls::Kvm;
use vm::Guest;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("USAGE: {} <guest-image>", args[0]);
        std::process::exit(1);
    }

    let mut kvm = Kvm::new().unwrap();
    let mut guest = Guest::new(&mut kvm, &args[1]);
    guest.run();
}
