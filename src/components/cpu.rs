use super::assembly::{Assembler, OpCode};
use super::bus::BUS;

bitflags! {
    /// # Status Register (P) http://wiki.nesdev.com/w/index.php/Status_flags
    ///
    ///  7 6 5 4 3 2 1 0
    ///  N V _ B D I Z C
    ///  | |   | | | | +--- Carry Flag
    ///  | |   | | | +----- Zero Flag
    ///  | |   | | +------- Interrupt Disable
    ///  | |   | +--------- Decimal Mode (not used on NES)
    ///  | |   +----------- Break Command
    ///  | +--------------- Overflow Flag
    ///  +----------------- Negative Flag
    ///
    pub struct CpuFlags: u8 {
        const CARRY             = 0b00000001;
        const ZERO              = 0b00000010;
        const INTERRUPT_DISABLE = 0b00000100;
        const DECIMAL_MODE      = 0b00001000;
        const BREAK             = 0b00010000;
        const UNUSED            = 0b00100000;
        const OVERFLOW          = 0b01000000;
        const NEGATIVE          = 0b10000000;
    }
}

const STACK: u16 = 0x0100;
const STACK_RESET: u8 = 0xfd;

pub struct CPU<'a> {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub register_p: CpuFlags,
    pub register_pc: u16,
    pub register_sp: u8,
    pub bus: BUS<'a>,
}

#[derive(Debug)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    IndirectX,
    IndirectY,
    NoneAddressing,
}

fn page_cross(addr1: u16, addr2: u16) -> bool {
    addr1 & 0xFF00 != addr2 & 0xFF00
}

#[derive(PartialEq, Eq)]
pub enum InterruptType {
    NMI,
}

#[derive(PartialEq, Eq)]
pub struct Interrupt {
    pub interrupt_type: InterruptType,
    pub vector_address: u16,
    pub binary_flag_mask: u8,
    pub cpu_cycles: u8,
}

pub const NMI: Interrupt = Interrupt {
    interrupt_type: InterruptType::NMI,
    vector_address: 0xfffA,
    binary_flag_mask: 0b00100000,
    cpu_cycles: 2,
};

impl<'a> CPU<'a> {
    pub fn new<'b>(bus: BUS<'b>) -> CPU<'b> {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            register_sp: STACK_RESET,
            register_pc: 0,
            register_p: CpuFlags::from_bits_truncate(0b100100),
            bus,
        }
    }

    pub fn memory_read(&mut self, address: u16) -> u8 {
        self.bus.memory_read(address)
    }

    pub fn memory_write(&mut self, address: u16, value: u8) {
        self.bus.memory_write(address, value)
    }

    pub fn memory_read_u16(&mut self, address: u16) -> u16 {
        self.bus.memory_read_u16(address)
    }

    // fn memory_write_u16(&mut self, address: u16, value: u16) {
    //     self.bus.memory_write_u16(address, value)
    // }

    // returns (address, page_cross flag)
    pub fn get_absolute_address(&mut self, mode: &AddressingMode, address: u16) -> (u16, bool) {
        match mode {
            AddressingMode::ZeroPage => (self.memory_read(address) as u16, false),

            AddressingMode::Absolute => (self.memory_read_u16(address), false),

            AddressingMode::ZeroPageX => {
                let index = self.memory_read(address);
                let address = index.wrapping_add(self.register_x) as u16;
                (address, false)
            }
            AddressingMode::ZeroPageY => {
                let index = self.memory_read(address);
                let address = index.wrapping_add(self.register_y) as u16;
                (address, false)
            }

            AddressingMode::AbsoluteX => {
                let base = self.memory_read_u16(address);
                let address = base.wrapping_add(self.register_x as u16);
                (address, page_cross(base, address))
            }
            AddressingMode::AbsoluteY => {
                let base = self.memory_read_u16(address);
                let address = base.wrapping_add(self.register_y as u16);
                (address, page_cross(base, address))
            }

            AddressingMode::IndirectX => {
                let base = self.memory_read(address);

                let ptr: u8 = (base as u8).wrapping_add(self.register_x);
                let lo = self.memory_read(ptr as u16);
                let hi = self.memory_read(ptr.wrapping_add(1) as u16);
                ((hi as u16) << 8 | (lo as u16), false)
            }
            AddressingMode::IndirectY => {
                let base = self.memory_read(address);

                let lo = self.memory_read(base as u16);
                let hi = self.memory_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                (deref, page_cross(deref, deref_base))
            }

            _ => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    pub fn get_operand_address(&mut self, mode: &AddressingMode) -> (u16, bool) {
        match mode {
            AddressingMode::Immediate => (self.register_pc, false),
            _ => self.get_absolute_address(mode, self.register_pc),
        }
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
        if result == 0 {
            self.register_p.insert(CpuFlags::ZERO);
        } else {
            self.register_p.remove(CpuFlags::ZERO);
        }

        if result >> 7 == 1 {
            self.register_p.insert(CpuFlags::NEGATIVE);
        } else {
            self.register_p.remove(CpuFlags::NEGATIVE);
        }
    }

    fn update_negative_flags(&mut self, result: u8) {
        if result >> 7 == 1 {
            self.register_p.insert(CpuFlags::NEGATIVE)
        } else {
            self.register_p.remove(CpuFlags::NEGATIVE)
        }
    }

    fn set_carry_flag(&mut self) {
        self.register_p.insert(CpuFlags::CARRY)
    }

    fn clear_carry_flag(&mut self) {
        self.register_p.remove(CpuFlags::CARRY)
    }

    fn set_register_a(&mut self, value: u8) {
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    /// note: ignoring decimal mode
    /// http://www.righto.com/2012/12/the-6502-overflow-flag-explained.html
    fn add_to_register_a(&mut self, data: u8) {
        let sum = self.register_a as u16
            + data as u16
            + (if self.register_p.contains(CpuFlags::CARRY) {
                1
            } else {
                0
            }) as u16;

        let carry = sum > 0xff;

        if carry {
            self.register_p.insert(CpuFlags::CARRY);
        } else {
            self.register_p.remove(CpuFlags::CARRY);
        }

        let result = sum as u8;

        if (data ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.register_p.insert(CpuFlags::OVERFLOW);
        } else {
            self.register_p.remove(CpuFlags::OVERFLOW)
        }

        self.set_register_a(result);
    }

    fn sub_from_register_a(&mut self, data: u8) {
        self.add_to_register_a(((data as i8).wrapping_neg().wrapping_sub(1)) as u8);
    }

    fn stack_pop(&mut self) -> u8 {
        self.register_sp = self.register_sp.wrapping_add(1);
        self.memory_read((STACK as u16) + self.register_sp as u16)
    }

    fn stack_push(&mut self, data: u8) {
        self.memory_write((STACK as u16) + self.register_sp as u16, data);
        self.register_sp = self.register_sp.wrapping_sub(1)
    }

    fn stack_push_u16(&mut self, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.stack_push(hi);
        self.stack_push(lo);
    }

    fn stack_pop_u16(&mut self) -> u16 {
        let lo = self.stack_pop() as u16;
        let hi = self.stack_pop() as u16;

        hi << 8 | lo
    }

    pub fn update_pc(&mut self, opcode: &&OpCode, pc_state: u16) {
        self.bus.tick(opcode.cycles);

        if pc_state == self.register_pc {
            self.register_pc += (opcode.len - 1) as u16;
        }
    }

    pub fn adc(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(mode);
        let value = self.memory_read(address);
        self.add_to_register_a(value);

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn and(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(mode);
        let value = self.memory_read(address);
        self.set_register_a(value & self.register_a);

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn asl_accumulator(&mut self) {
        let mut value = self.register_a;
        if value >> 7 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        value <<= 1;
        self.set_register_a(value)
    }

    pub fn asl(&mut self, mode: &AddressingMode) -> u8 {
        let (address, _) = self.get_operand_address(mode);
        let mut value = self.memory_read(address);
        if value >> 7 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        value <<= 1;
        self.memory_write(address, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    pub fn bcc(&mut self) {
        self.branch(!self.register_p.contains(CpuFlags::CARRY));
    }

    pub fn bcs(&mut self) {
        self.branch(self.register_p.contains(CpuFlags::CARRY));
    }

    pub fn beq(&mut self) {
        self.branch(self.register_p.contains(CpuFlags::ZERO));
    }

    pub fn bit(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let value = self.memory_read(address);
        let and = self.register_a & value;
        if and == 0 {
            self.register_p.insert(CpuFlags::ZERO);
        } else {
            self.register_p.remove(CpuFlags::ZERO);
        }

        self.register_p
            .set(CpuFlags::NEGATIVE, value & 0b10000000 > 0);
        self.register_p
            .set(CpuFlags::OVERFLOW, value & 0b01000000 > 0);
    }

    pub fn bmi(&mut self) {
        self.branch(self.register_p.contains(CpuFlags::NEGATIVE));
    }

    pub fn bne(&mut self) {
        self.branch(!self.register_p.contains(CpuFlags::ZERO));
    }

    pub fn bpl(&mut self) {
        self.branch(!self.register_p.contains(CpuFlags::NEGATIVE));
    }

    // BRK is a simple return in Assembler interpreter function

    pub fn bvc(&mut self) {
        self.branch(!self.register_p.contains(CpuFlags::OVERFLOW));
    }

    pub fn bvs(&mut self) {
        self.branch(self.register_p.contains(CpuFlags::OVERFLOW));
    }

    pub fn clc(&mut self) {
        self.clear_carry_flag();
    }

    pub fn cld(&mut self) {
        self.register_p.remove(CpuFlags::DECIMAL_MODE);
    }

    pub fn cli(&mut self) {
        self.register_p.remove(CpuFlags::INTERRUPT_DISABLE);
    }

    pub fn clv(&mut self) {
        self.register_p.remove(CpuFlags::OVERFLOW);
    }

    pub fn cmp(&mut self, mode: &AddressingMode) {
        self.compare(mode, self.register_a);
    }

    pub fn cpx(&mut self, mode: &AddressingMode) {
        self.compare(mode, self.register_x);
    }

    pub fn cpy(&mut self, mode: &AddressingMode) {
        self.compare(mode, self.register_y);
    }

    pub fn dec(&mut self, mode: &AddressingMode) -> u8 {
        let (address, _) = self.get_operand_address(mode);
        let mut value = self.memory_read(address);
        value = value.wrapping_sub(1);
        self.memory_write(address, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    pub fn dex(&mut self) {
        self.register_x = self.register_x.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    pub fn dey(&mut self) {
        self.register_y = self.register_y.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    pub fn eor(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(mode);
        let value = self.memory_read(address);
        self.set_register_a(value ^ self.register_a);

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn inc(&mut self, mode: &AddressingMode) -> u8 {
        let (address, _) = self.get_operand_address(mode);
        let mut value = self.memory_read(address);
        value = value.wrapping_add(1);
        self.memory_write(address, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    pub fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    pub fn iny(&mut self) {
        self.register_y = self.register_y.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    pub fn jmp_absolute(&mut self) {
        let memory_address = self.memory_read_u16(self.register_pc);
        self.register_pc = memory_address;
    }

    pub fn jmp_indirect(&mut self) {
        let memory_address = self.memory_read_u16(self.register_pc);

        let indirect_reference = if memory_address & 0x00FF == 0x00FF {
            let low = self.memory_read(memory_address);
            let high = self.memory_read(memory_address & 0xFF00);
            (high as u16) << 8 | (low as u16)
        } else {
            self.memory_read_u16(memory_address)
        };

        self.register_pc = indirect_reference;
    }

    pub fn jsr(&mut self) {
        self.stack_push_u16(self.register_pc + 2 - 1);
        let target_address = self.memory_read_u16(self.register_pc);

        self.register_pc = target_address;
    }

    pub fn lda(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(&mode);
        let value = self.memory_read(address);
        self.set_register_a(value);

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn ldx(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(mode);
        let value = self.memory_read(address);

        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn ldy(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(mode);
        let value = self.memory_read(address);

        self.register_y = value;
        self.update_zero_and_negative_flags(self.register_y);

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn lsr_accumulator(&mut self) {
        let mut value = self.register_a;
        if value & 1 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        value = value >> 1;
        self.set_register_a(value)
    }

    pub fn lsr(&mut self, mode: &AddressingMode) -> u8 {
        let (address, _) = self.get_operand_address(mode);
        let mut value = self.memory_read(address);
        if value & 1 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        value >>= 1;
        self.memory_write(address, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    // NOP is a simple {} in Assembler interpret function

    pub fn ora(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(mode);
        let value = self.memory_read(address);
        self.set_register_a(value | self.register_a);

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn pha(&mut self) {
        self.stack_push(self.register_a);
    }

    pub fn php(&mut self) {
        let mut flags = self.register_p.clone();
        flags.insert(CpuFlags::BREAK);
        flags.insert(CpuFlags::UNUSED);
        self.stack_push(flags.bits());
    }

    pub fn pla(&mut self) {
        let value = self.stack_pop();
        self.set_register_a(value);
    }

    pub fn plp(&mut self) {
        self.register_p.bits = self.stack_pop();
        self.register_p.remove(CpuFlags::BREAK);
        self.register_p.insert(CpuFlags::UNUSED);
    }

    pub fn rol_accumulator(&mut self) {
        let mut value = self.register_a;
        let old_carry = self.register_p.contains(CpuFlags::CARRY);

        if value >> 7 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        value <<= 1;
        if old_carry {
            value = value | 1;
        }
        self.set_register_a(value);
    }

    pub fn rol(&mut self, mode: &AddressingMode) -> u8 {
        let (address, _) = self.get_operand_address(mode);
        let mut value = self.memory_read(address);
        let old_carry = self.register_p.contains(CpuFlags::CARRY);

        if value >> 7 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        value <<= 1;

        if old_carry {
            value |= 1;
        }

        self.memory_write(address, value);
        self.update_negative_flags(value);
        value
    }

    pub fn ror_accumulator(&mut self) {
        let mut value = self.register_a;
        let old_carry = self.register_p.contains(CpuFlags::CARRY);

        if value & 1 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        value >>= 1;

        if old_carry {
            value |= 0b10000000;
        }

        self.set_register_a(value);
    }

    pub fn ror(&mut self, mode: &AddressingMode) -> u8 {
        let (address, _) = self.get_operand_address(mode);
        let mut value = self.memory_read(address);
        let old_carry = self.register_p.contains(CpuFlags::CARRY);

        if value & 1 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        value >>= 1;

        if old_carry {
            value = value | 0b10000000;
        }
        self.memory_write(address, value);
        self.update_negative_flags(value);
        value
    }

    pub fn rti(&mut self) {
        self.register_p.bits = self.stack_pop();
        self.register_p.remove(CpuFlags::BREAK);
        self.register_p.insert(CpuFlags::UNUSED);

        self.register_pc = self.stack_pop_u16();
    }

    pub fn rts(&mut self) {
        self.register_pc = self.stack_pop_u16() + 1;
    }

    pub fn sbc(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(&mode);
        let value = self.memory_read(address);
        self.add_to_register_a(((value as i8).wrapping_neg().wrapping_sub(1)) as u8);

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn sec(&mut self) {
        self.set_carry_flag();
    }

    pub fn sed(&mut self) {
        self.register_p.insert(CpuFlags::DECIMAL_MODE);
    }

    pub fn sei(&mut self) {
        self.register_p.insert(CpuFlags::INTERRUPT_DISABLE);
    }

    pub fn sta(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        self.memory_write(address, self.register_a);
    }

    pub fn stx(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        self.memory_write(address, self.register_x);
    }

    pub fn sty(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        self.memory_write(address, self.register_y);
    }

    pub fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    pub fn tay(&mut self) {
        self.register_y = self.register_a;
        self.update_zero_and_negative_flags(self.register_y);
    }

    pub fn tsx(&mut self) {
        self.register_x = self.register_sp;
        self.update_zero_and_negative_flags(self.register_x);
    }

    pub fn txa(&mut self) {
        self.register_a = self.register_x;
        self.update_zero_and_negative_flags(self.register_a);
    }

    pub fn txs(&mut self) {
        self.register_sp = self.register_x;
    }

    pub fn tya(&mut self) {
        self.register_a = self.register_y;
        self.update_zero_and_negative_flags(self.register_a);
    }

    // unofficial opcodes

    pub fn dcp(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let mut value = self.memory_read(address);

        value = value.wrapping_sub(value);

        self.memory_write(address, value);
        if value <= self.register_a {
            self.register_p.insert(CpuFlags::CARRY);
        }

        self.update_zero_and_negative_flags(value.wrapping_sub(value));
    }

    pub fn rla(&mut self, mode: &AddressingMode) {
        let value = self.rol(mode);
        self.set_register_a(value & self.register_a);
    }

    pub fn slo(&mut self, mode: &AddressingMode) {
        let value = self.asl(mode);
        self.set_register_a(value | self.register_a);
    }

    pub fn sre(&mut self, mode: &AddressingMode) {
        let value = self.lsr(mode);
        self.set_register_a(value ^ self.register_a);
    }

    // skb is a 2 byte NOP immediate

    pub fn axs(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let value = self.memory_read(address);

        let x_and_a = self.register_x & self.register_a;
        let result = x_and_a.wrapping_sub(value);

        if value <= x_and_a {
            self.register_p.insert(CpuFlags::CARRY);
        }

        self.update_zero_and_negative_flags(result);

        self.register_x = result;
    }

    pub fn arr(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let value = self.memory_read(address);

        self.set_register_a(value & self.register_a);
        self.ror_accumulator();

        let result = self.register_a;
        let bit_5 = (result >> 5) & 1;
        let bit_6 = (result >> 6) & 1;

        if bit_6 == 1 {
            self.register_p.insert(CpuFlags::CARRY)
        } else {
            self.register_p.remove(CpuFlags::CARRY)
        }

        if bit_5 ^ bit_6 == 1 {
            self.register_p.insert(CpuFlags::OVERFLOW);
        } else {
            self.register_p.remove(CpuFlags::OVERFLOW);
        }

        self.update_zero_and_negative_flags(result);
    }

    pub fn unofficial_sbc(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let value = self.memory_read(address);

        self.sub_from_register_a(value);
    }

    pub fn anc(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let value = self.memory_read(address);

        self.set_register_a(value & self.register_a);

        if self.register_p.contains(CpuFlags::NEGATIVE) {
            self.register_p.insert(CpuFlags::CARRY);
        } else {
            self.register_p.remove(CpuFlags::CARRY);
        }
    }

    pub fn alr(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let value = self.memory_read(address);

        self.set_register_a(value & self.register_a);
        self.lsr_accumulator();
    }

    pub fn nop_read(&mut self, mode: &AddressingMode) {
        let (address, page_cross) = self.get_operand_address(mode);
        let _value = self.memory_read(address);

        if page_cross {
            self.bus.tick(1);
        }

        // do nothing
    }

    pub fn rra(&mut self, mode: &AddressingMode) {
        let value = self.ror(mode);
        self.add_to_register_a(value);
    }

    pub fn isb(&mut self, mode: &AddressingMode) {
        let value = self.inc(mode);
        self.sub_from_register_a(value);
    }

    // all unofficial NOP'S are just {} in assembly code

    pub fn lax(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let value = self.memory_read(address);
        self.set_register_a(value);
        self.register_x = self.register_a;
    }

    pub fn sax(&mut self, mode: &AddressingMode) {
        let value = self.register_a & self.register_x;
        let (address, _) = self.get_operand_address(mode);

        self.memory_write(address, value);
    }

    pub fn lxa(&mut self, mode: &AddressingMode) {
        self.lda(mode);
        self.tax();
    }

    pub fn xaa(&mut self, mode: &AddressingMode) {
        self.register_a = self.register_x;
        self.update_zero_and_negative_flags(self.register_a);

        let (address, _) = self.get_operand_address(mode);
        let value = self.memory_read(address);

        self.set_register_a(value & self.register_a);
    }

    pub fn las(&mut self, mode: &AddressingMode) {
        let (address, _) = self.get_operand_address(mode);
        let mut value = self.memory_read(address);

        value &= self.register_sp;

        self.register_a = value;
        self.register_x = value;
        self.register_sp = value;

        self.update_zero_and_negative_flags(value);
    }

    pub fn tas(&mut self) {
        let x_and_a = self.register_x & self.register_a;
        self.register_sp = x_and_a;

        let address = self.memory_read_u16(self.register_pc);
        let address = address + self.register_y as u16;

        let high_plus_1 = (address >> 8) as u8 + 1;

        let value = high_plus_1 & self.register_sp;

        self.memory_write(address, value);
    }

    pub fn axa_indirect(&mut self) {
        let position = self.memory_read(self.register_pc);
        let address = self.memory_read_u16(position as u16);

        let address = address + self.register_y as u16;
        let x_and_a = self.register_x & self.register_a;

        let high = (address >> 8) as u8;
        let value = x_and_a & high;

        self.memory_write(address, value);
    }

    pub fn axa_absolute(&mut self) {
        let address = self.memory_read_u16(self.register_pc);
        let address = address + self.register_y as u16;

        let x_and_a = self.register_x & self.register_a;
        let high = (address >> 8) as u8;

        let value = x_and_a & high;
        self.memory_write(address, value);
    }

    pub fn sxa(&mut self) {
        let address = self.memory_read_u16(self.register_pc);
        let address = address + self.register_y as u16;

        let high_plus_1 = (address >> 8) as u8 + 1;
        let value = self.register_x & high_plus_1;

        self.memory_write(address, value);
    }

    pub fn sya(&mut self) {
        let address = self.memory_read_u16(self.register_pc);
        let address = address + self.register_x as u16;

        let high_plus_1 = (address >> 8) as u8 + 1;
        let value = self.register_x & high_plus_1;

        self.memory_write(address, value);
    }

    fn branch(&mut self, condition: bool) {
        if condition {
            self.bus.tick(1);

            let jump: i8 = self.memory_read(self.register_pc) as i8;
            let jump_addr = self.register_pc.wrapping_add(1).wrapping_add(jump as u16);

            if self.register_pc.wrapping_add(1) & 0xFF00 != jump_addr & 0xFF00 {
                self.bus.tick(1);
            }

            self.register_pc = jump_addr;
        }
    }

    fn compare(&mut self, mode: &AddressingMode, compare_with: u8) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.memory_read(addr);
        if data <= compare_with {
            self.register_p.insert(CpuFlags::CARRY);
        } else {
            self.register_p.remove(CpuFlags::CARRY);
        }

        self.update_zero_and_negative_flags(compare_with.wrapping_sub(data));

        if page_cross {
            self.bus.tick(1);
        }
    }

    pub fn interrupt(&mut self, interrupt: Interrupt) {
        self.stack_push_u16(self.register_pc);
        let mut flag = self.register_p.clone();
        flag.set(CpuFlags::BREAK, interrupt.binary_flag_mask & 0b010000 == 1);
        flag.set(CpuFlags::UNUSED, interrupt.binary_flag_mask & 0b100000 == 1);

        self.stack_push(flag.bits);
        self.register_p.insert(CpuFlags::INTERRUPT_DISABLE);

        self.bus.tick(interrupt.cpu_cycles);
        self.register_pc = self.memory_read_u16(interrupt.vector_address);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.register_pc = 0x0600;
        self.run()
    }

    pub fn load(&mut self, program: Vec<u8>) {
        for i in 0..(program.len() as u16) {
            self.memory_write(0x0600 + i, program[i as usize]);
        }
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.register_sp = STACK_RESET;
        self.register_p = CpuFlags::from_bits_truncate(0b100100);

        self.register_pc = self.memory_read_u16(0xFFFC);
    }

    pub fn run(&mut self) {
        self.run_with_callback(|_| {});
    }

    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut CPU),
    {
        let assembler = Assembler::new();

        loop {
            if let Some(_nmi) = self.bus.poll_nmi_status() {
                self.interrupt(NMI);
            }

            let code = self.memory_read(self.register_pc);
            self.register_pc += 1;

            let program_ends = assembler.interpret(self, code);

            if program_ends {
                break;
            } else {
                callback(self);
            }
        }
    }
}
