# Copy from https://zserge.com/posts/kvm/

# A tiny 16-bit guest "kernel" that infinitely prints an incremented number to the debug port

#
# Build it:
#
# as -32 guest1.S -o guest1.o
#	ld -m elf_i386 --oformat binary -N -e _start -Ttext 0x10000 -o guest1.bin guest1.o
#
# disas: objdump -b binary -D examples/guest1 -m i386

.globl _start
.code16
_start:
  xorw %ax, %ax
loop:
	out %ax, $0x10
	inc %ax
	jmp loop
