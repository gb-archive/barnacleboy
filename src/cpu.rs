use memory::MemoryInterface;
use std::{cell::RefCell, rc::Rc};
/// Zero flag.
const F_ZERO: u8 = 0b1000_0000;
/// Subtraction flag.
const F_SUBTRACT: u8 = 0b0100_0000;
/// Half-carry flag.
const F_HALFCARRY: u8 = 0b0010_0000;
/// Carry flag.
const F_CARRY: u8 = 0b0001_0000;

/// Trait defining the interface to the CPU.
pub trait Cpu {
    type Error;

    /// Execute opcodes until stopped.
    fn execute(&mut self) -> Result<(), Self::Error>;
    /// Execute opcodes until greater than or equal to `cycle_bound`.
    fn execute_with_cycles(&mut self, cycle_bound: usize) -> Result<(), Self::Error>;
    /// Executes a single instruction, returning the number of cycles executed
    /// by the instruction.
    fn step(&mut self) -> Result<usize, Self::Error>;
}

type MemoryController = Rc<RefCell<MemoryInterface<Word = u8, Index = u16, Error = LRError>>>;
type SharpResult = Result<(), LRError>;
/// The original Sharp LR35902 processor, a 8080/Z80 derivative with some
/// interesting changes. Most notably, removal of the shadow register set along
/// with various opcode changes.
pub struct SharpLR35902 {
    registers: SharpLR35902Registers,
    interrupt_pending: bool,
    halted: bool,
    memory_controller: MemoryController,
}

/// The Sharp LR35902 register set. Contains 7 8-bit general purpose registers
/// (`a`, `b`, `c`, `d`, `e`, `h`, `l`), a flag register, program counter, and
/// stack pointer. Each of the 7 general purpose registers plus the flags
/// register can be combined into 4 different 16-bit register pairs: `af`, `bc`,
/// `de`, and `hl`. The first register name denotes the top 8 bits, with the
/// second denoting the bottom 8 bits.
#[cfg(target_endian = "little")]
#[derive(Debug, Default, Clone, Copy)]
#[repr(C, align(2))]
struct SharpLR35902Registers {
    pub f: u8,
    pub a: u8,
    pub c: u8,
    pub b: u8,
    pub e: u8,
    pub d: u8,
    pub l: u8,
    pub h: u8,
    pub pc: u16,
    pub sp: u16,
}

#[cfg(target_endian = "big")]
#[derive(Debug, Default, Clone, Copy)]
#[repr(C, align(2))]
struct SharpLR35902Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub pc: u16,
    pub sp: u16,
}

impl SharpLR35902Registers {
    /// Temporarily transmutes `SharpLR35902Registers` into `DWordRegisters`
    /// which contain the 16-bit register pairs for convenience.
    fn as_dwords(&mut self) -> &mut DWordRegisters {
        unsafe { &mut *(self as *mut SharpLR35902Registers as *mut DWordRegisters) }
    }

    /// Sets the zero flag.
    fn set_z(&mut self) {
        self.f |= F_ZERO;
    }

    /// Sets the subtraction flag.
    fn set_s(&mut self) {
        self.f |= F_SUBTRACT;
    }

    /// Sets the half-carry flag.
    fn set_h(&mut self) {
        self.f |= F_HALFCARRY;
    }

    /// Sets the carry flag.
    fn set_c(&mut self) {
        self.f |= F_CARRY;
    }

    /// Clears the zero flag.
    fn clear_z(&mut self) {
        self.f &= !F_ZERO;
    }

    /// Clears the subtraction flag.
    fn clear_s(&mut self) {
        self.f &= !F_SUBTRACT;
    }

    /// Clears the half-carry flag.
    fn clear_h(&mut self) {
        self.f &= !F_HALFCARRY;
    }

    /// Clears the carry flag.
    fn clear_c(&mut self) {
        self.f &= !F_CARRY;
    }

    /// Returns whether or not the zero flag is set.
    fn z(&self) -> bool {
        self.f & F_ZERO == F_ZERO
    }

    /// Returns whether or not the subtraction flag is set.
    fn s(&self) -> bool {
        self.f & F_SUBTRACT == F_SUBTRACT
    }

    /// Returns whether or not the half-carry flag is set.
    fn h(&self) -> bool {
        self.f & F_HALFCARRY == F_HALFCARRY
    }

    /// Returns whether or not the carry flag is set.
    fn c(&self) -> bool {
        self.f & F_CARRY == F_CARRY
    }
}

/// Representation of the 16-bit register pairs.
#[derive(Clone, Copy, Default, Debug)]
#[repr(C, align(2))]
struct DWordRegisters {
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub pc: u16,
    pub sp: u16,
}

impl ::std::ops::Index<u8> for SharpLR35902Registers {
    type Output = u8;

    fn index(&self, index: u8) -> &u8 {
        match index {
            0 => &self.b,
            1 => &self.c,
            2 => &self.d,
            3 => &self.e,
            4 => &self.h,
            5 => &self.l,
            6 => &self.f,
            7 => &self.a,
            _ => panic!("Register index out of bounds."),
        }
    }
}

impl ::std::ops::IndexMut<u8> for SharpLR35902Registers {
    fn index_mut(&mut self, index: u8) -> &mut u8 {
        match index {
            0 => &mut self.b,
            1 => &mut self.c,
            2 => &mut self.d,
            3 => &mut self.e,
            4 => &mut self.h,
            5 => &mut self.l,
            6 => &mut self.f,
            7 => &mut self.a,
            _ => panic!("Register index out of bounds."),
        }
    }
}

impl ::std::ops::Index<u8> for DWordRegisters {
    type Output = u16;

    fn index(&self, index: u8) -> &u16 {
        match index {
            0 => &self.bc,
            1 => &self.de,
            2 => &self.hl,
            3 => &self.af,
            _ => panic!("DWord register index out of bounds."),
        }
    }
}

impl ::std::ops::IndexMut<u8> for DWordRegisters {
    fn index_mut(&mut self, index: u8) -> &mut u16 {
        match index {
            0 => &mut self.bc,
            1 => &mut self.de,
            2 => &mut self.hl,
            3 => &mut self.af,
            _ => panic!("DWord register index out of bounds."),
        }
    }
}

/// CPU errors.
#[derive(Debug)]
pub enum LRError {
    InvalidMemoryRead(u16),
    InvalidMemoryWrite(u16),
}

impl ::std::fmt::Display for LRError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LRError::InvalidMemoryRead(addr) => format!("Invalid memory read at {:X}", addr),
                LRError::InvalidMemoryWrite(addr) => format!("Invalid memory write at {:X}", addr),
            }
        )
    }
}

impl ::std::error::Error for LRError {
    fn description(&self) -> &'static str {
        match self {
            LRError::InvalidMemoryRead(_) => "Invalid memory read",
            LRError::InvalidMemoryWrite(_) => "Invalid memory write",
        }
    }
}

impl Cpu for SharpLR35902 {
    type Error = LRError;
    fn execute(&mut self) -> Result<(), LRError> {
        unimplemented!()
    }

    fn execute_with_cycles(&mut self, cycle_bound: usize) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn step(&mut self) -> Result<usize, Self::Error> {
        unimplemented!()
    }
}

impl SharpLR35902 {
    /// Creates a new `SharpLR35902` from an `Rc<RefCell<MemoryInterface>>`.
    pub fn new(mi: MemoryController) -> SharpLR35902 {
        Self {
            registers: Default::default(),
            interrupt_pending: false,
            halted: false,
            memory_controller: mi,
        }
    }

    /// Reads a byte at the program counter and increments.
    fn read_instruction_byte(&mut self) -> Result<u8, LRError> {
        let pc = self.registers.pc;
        self.registers.pc += 1;

        Ok(self.memory_controller.borrow().read(pc)?)
    }

    /// Reads a byte at the address pointed to by `HL`.
    fn read_hl(&mut self) -> Result<u8, LRError> {
        let hl = self.registers.as_dwords().hl;

        Ok(self.memory_controller.borrow().read(hl)?)
    }

    /// Writes a byte at the given address.
    fn write(&mut self, addr: u16, data: u8) -> Result<(), LRError> {
        self.memory_controller.borrow_mut().write(addr, data)?;

        Ok(())
    }

    /// Writes a byte to the address pointed to by `HL`.
    fn write_hl(&mut self, data: u8) -> Result<(), LRError> {
        let hl = self.registers.as_dwords().hl;

        self.memory_controller.borrow_mut().write(hl, data)?;
        Ok(())
    }
}

struct OpcodeBits {
    x: u8,
    y: u8,
    z: u8,
    p: u8,
    q: u8,
}

impl From<u8> for OpcodeBits {
    fn from(op: u8) -> OpcodeBits {
        let x = (op & 0b1100_0000) >> 6;
        let y = (op & 0b0011_1000) >> 3;
        let z = op & 0b0000_0111;
        let p = (y & 0b110) >> 1;
        let q = y & 0b001;

        OpcodeBits { x, y, z, p, q }
    }
}

const INSTRUCTIONS: [fn(&mut SharpLR35902, u8) -> SharpResult; 2] = [nop, ld_r_r];

fn nop(_: &mut SharpLR35902, _: u8) -> SharpResult {
    Ok(())
}

fn ld_r_r(cpu: &mut SharpLR35902, opcode: u8) -> SharpResult {
    let bits = OpcodeBits::from(opcode);
    cpu.registers[bits.y] = cpu.registers[bits.z];

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DummyMemInterface;

    impl MemoryInterface for DummyMemInterface {
        type Word = u8;
        type Index = u16;
        type Error = LRError;

        fn read(&self, _address: Self::Index) -> Result<Self::Word, Self::Error> {
            unimplemented!()
        }
        fn write(&mut self, _address: Self::Index, _data: Self::Word) -> Result<(), Self::Error> {
            unimplemented!()
        }
    }

    #[test]
    fn flags() {
        let mut cpu = SharpLR35902::new(Rc::new(RefCell::new(DummyMemInterface {})));

        cpu.registers.set_c();
        assert!(cpu.registers.c());

        cpu.registers.set_s();
        assert!(cpu.registers.s());

        cpu.registers.set_z();
        assert!(cpu.registers.z());

        cpu.registers.set_h();
        assert!(cpu.registers.h());

        cpu.registers.clear_c();
        assert!(!cpu.registers.c());

        cpu.registers.clear_s();
        assert!(!cpu.registers.s());

        cpu.registers.clear_z();
        assert!(!cpu.registers.z());

        cpu.registers.clear_h();
        assert!(!cpu.registers.h());
    }

    #[test]
    fn registers() {
        let mut cpu = SharpLR35902::new(Rc::new(RefCell::new(DummyMemInterface {})));

        cpu.registers.a = 0x11;
        cpu.registers.f = 0x22;
        cpu.registers.b = 0x33;
        cpu.registers.c = 0x44;
        cpu.registers.d = 0x55;
        cpu.registers.e = 0x66;
        cpu.registers.h = 0x77;
        cpu.registers.l = 0x88;

        assert_eq!(cpu.registers.as_dwords().af, 0x1122);
        assert_eq!(cpu.registers.as_dwords().bc, 0x3344);
        assert_eq!(cpu.registers.as_dwords().de, 0x5566);
        assert_eq!(cpu.registers.as_dwords().hl, 0x7788);
    }

    #[test]
    fn opcode_bits() {
        let opcode = 0b1010_1010;
        let bits = OpcodeBits::from(opcode);

        assert_eq!(bits.x, 0b10);
        assert_eq!(bits.y, 0b101);
        assert_eq!(bits.z, 0b010);
        assert_eq!(bits.p, 0b10);
        assert_eq!(bits.q, 0b1);
    }
}
