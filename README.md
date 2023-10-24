# Simple VMM

A tiny KVM-based virtual machine monitor which boots Linux kernel.

## Prerequisites

- x86-64
- Cargo

## Run

```bash
cargo run examples/test-bzImage
```

## History

- https://github.com/tamaroning/simple_vmm/commit/6ea33ef0a761c3e58026ed4c3046108996323a7e
    - Minimul implementation to boot Linux kernel (241 LOC)

## References

- https://lwn.net/Articles/658511/
- https://zserge.com/posts/kvm/
- https://github.com/bobuhiro11/gokvm
