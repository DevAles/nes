use std::collections::HashMap;
use crate::{ opcodes, memmory::Memmory };

pub const ROM_FIRST_BYTE: u16 = 0x8000;
pub const ROM_FIRST_ADDRESS: u16 = 0xFFFC;

#[derive(Debug)]
pub enum AddressingMode {
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Relative,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    NoneAddressing,
}

#[derive(Debug)]
pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub register_sr: u8,
    pub register_pc: u16,
    pub stack: Vec<u8>,
    pub memmory: Memmory,
}

impl Default for CPU {
    fn default() -> Self {
        Self::new()
    }
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0x00,
            register_x: 0x00,
            register_y: 0x00,
            register_sr: 0b0000_0000,
            register_pc: 0x00,
            memmory: Memmory::new(),
            stack: Vec::new(),
        }
    }

    fn get_operand_address(&self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Accumulator => self.register_a as u16,
            AddressingMode::Immediate => self.register_pc, // Get the address into register, not the value.
            AddressingMode::ZeroPage => self.memmory.read(self.register_pc) as u16, // Get any value less then 256 bytes.
            AddressingMode::Absolute => self.memmory.read_u16(self.register_pc),    // Loads any value.

            // Gets any value less then 256 bytes and add value of X register.
            AddressingMode::ZeroPageX => {
                let position = self.memmory.read(self.register_pc);
                position.wrapping_add(self.register_x) as u16
            }

            // Gets any value less then 256 bytes and add value of Y register.
            AddressingMode::ZeroPageY => {
                let position = self.memmory.read(self.register_pc);
                position.wrapping_add(self.register_y) as u16
            }

            // Gets any address and add in PC.
            AddressingMode::Relative => {
                let position = self.memmory.read(self.register_pc);
                self.register_pc.wrapping_add(position as u16)
            }

            // Gets any value and add value of X register.
            AddressingMode::AbsoluteX => {
                let base = self.memmory.read_u16(self.register_pc);
                base.wrapping_add(self.register_x as u16)
            }

            // Gets any value and add value of Y register.
            AddressingMode::AbsoluteY => {
                let base = self.memmory.read_u16(self.register_pc);
                base.wrapping_add(self.register_y as u16)
            }

            // Gets any value of any address.
            AddressingMode::Indirect => {
                let base = self.memmory.read_u16(self.register_pc);
                self.memmory.read_u16(base)
            }

            // Add value of X register in a zero page address, gets the value in this address, and ordenate him using Little Endian
            AddressingMode::IndirectX => {
                let base = self.memmory.read(self.register_pc);
                let pointer = base.wrapping_add(self.register_x);
                let low = self.memmory.read(pointer as u16);
                let high = self.memmory.read(pointer.wrapping_add(1) as u16);

                u16::from_le_bytes([high, low])
            }

            // Dereference an zero page address using Little Endian and add the Y register in result.
            AddressingMode::IndirectY => {
                let base = self.memmory.read(self.register_pc);
                let low = self.memmory.read(base as u16);
                let high = self.memmory.read(base.wrapping_add(1) as u16);
                let deref_base = u16::from_le_bytes([high, low]);

                deref_base.wrapping_add(self.register_y as u16)
            }

            AddressingMode::NoneAddressing => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    pub fn load(&mut self, program: Vec<u8>) {
        let program_space = ROM_FIRST_BYTE as usize..(ROM_FIRST_BYTE as usize + program.len());

        self.memmory.array[program_space].copy_from_slice(&program[..]);
        self.memmory.write_u16(ROM_FIRST_ADDRESS, ROM_FIRST_BYTE);
    }

    pub fn run(&mut self) {
        let opcodes: &HashMap<u8, &'static opcodes::OpCode> = &(*opcodes::OPCODES_HASHMAP);
        loop {
            let code = self.memmory.read(self.register_pc);
            self.register_pc += 1;
            let pc_state = self.register_pc;

            let opcode = opcodes.get(&code).unwrap_or_else(|| {
                panic!("OpCode {:x} is not recognized", code);
            });

            match code {
                0x69 | 0x65 | 0x75 | 0x6d | 0x7d | 0x79 | 0x61 | 0x71 => {
                    self.adc(&opcode.mode);
                }

                0x29 | 0x25 | 0x35 | 0x2d | 0x3d | 0x39 | 0x21 | 0x31 => {
                    self.and(&opcode.mode);
                }

                0x0a | 0x1a => {
                    self.asl(&opcode.mode);
                }

                0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => {
                    self.lda(&opcode.mode);
                }

                0xA2 | 0xA6 | 0xB6 | 0xAE | 0xBE => {
                    self.ldx(&opcode.mode);
                }

                0xA0 | 0xA4 | 0xB4 | 0xAC | 0xBC => {
                    self.ldy(&opcode.mode);
                }

                0x90 | 0xB0 | 0xF0 | 0x30 | 0xd0 | 0x10 | 0x50 | 0x70 => {
                    self.branch(&opcode.mode);
                }

                0x24 | 0x2C => {
                    self.bit(&opcode.mode);
                }

                0x18 => self.clc(),

                0xD8 => self.cld(),

                0x58 => self.cli(),

                0xB8 => self.clv(),

                0xC9 | 0xC5 | 0xD5 | 0xCD | 0xDD | 0xD9 | 0xC1 | 0xD1 => {
                    self.cmp(self.register_a, &opcode.mode);
                }

                0xE0 | 0xE4 | 0xEC => {
                    self.cmp(self.register_x, &opcode.mode);
                }

                0xC0 | 0xC4 | 0xCC => {
                    self.cmp(self.register_y, &opcode.mode);
                }

                0xC6 | 0xCE | 0xD6 | 0xDE => {
                    self.dec(&opcode.mode);
                }

                0xCA => self.dex(),

                0x88 => self.dey(),

                0x85 | 0x95 | 0x8d | 0x9D | 0x99 | 0x81 | 0x91 => {
                    self.sta(&opcode.mode);
                }

                0xE9 | 0xE5 | 0xF5 | 0xED | 0xFD | 0xF9 | 0xE1 | 0xF1 => {
                    self.sbc(&opcode.mode);
                }

                0xAA => self.tax(),
                0xE8 => self.inx(),
                0x48 => self.pha(),
                0x08 => self.php(),
                0x68 => self.pla(),
                0x28 => self.plp(),
                0x00 => return,
                _ => todo!(),
            }

            if pc_state == self.register_pc {
                self.register_pc += (opcode.len - 1) as u16
            }
        }
    }

    fn adc(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        if self.register_a > self.register_a.wrapping_add(value) {
            self.set_carry_flag(true);
        } else {
            self.set_carry_flag(false);
        }

        self.register_a = self.register_a.wrapping_add(value);

        self.update_zero_flag(self.register_a);
        self.update_negative_flag(self.register_a);
    }

    fn and(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        self.register_a &= value;
        self.update_zero_flag(self.register_a);
        self.update_negative_flag(self.register_a);
    }

    fn asl(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        self.register_a = value * 2;
        self.memmory.write(address, self.register_a);

        self.update_zero_flag(self.register_a);
        self.update_negative_flag(self.register_a);
    }

    fn branch(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        self.register_pc = value as u16;
    }

    fn bit(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        self.update_zero_flag(self.register_a & value);
        self.update_negative_flag(value);

        let overflow_flag = (value >> 6) & 1;

        if overflow_flag == 1 {
            self.set_overflow_flag(true);
        } else {
            self.set_overflow_flag(false);
        }
    }

    fn clc(&mut self) {
        self.set_carry_flag(false);
    }

    fn cld(&mut self) {
        self.set_decimal_flag(false);
    }

    fn cli(&mut self) {
        self.set_interrupt_disable_flag(false);
    }

    fn clv(&mut self) {
        self.set_overflow_flag(false);
    }

    fn cmp(&mut self, register: u8, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);
        let result = register.wrapping_sub(value);

        if register >= value {
            self.set_carry_flag(true);
        } else {
            self.set_carry_flag(false);
        }

        self.update_zero_flag(result);
        self.update_negative_flag(result);
    }

    fn dec(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        let result = value.wrapping_sub(1);
        self.memmory.write(address, result);

        self.update_zero_flag(result);
        self.update_negative_flag(result);
    }

    fn dex(&mut self) {
        self.register_x = self.register_x.wrapping_sub(1);
        self.update_zero_flag(self.register_x);
        self.update_negative_flag(self.register_x);
    }

    fn dey(&mut self) {
        self.register_y = self.register_y.wrapping_sub(1);
        self.update_zero_flag(self.register_y);
        self.update_negative_flag(self.register_y);
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        self.register_a = value;
        self.update_zero_flag(self.register_a);
        self.update_negative_flag(self.register_a);
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        self.register_x = value;
        self.update_zero_flag(self.register_x);
        self.update_negative_flag(self.register_x);
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        self.register_y = value;
        self.update_zero_flag(self.register_y);
        self.update_negative_flag(self.register_y);
    }

    fn sta(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        self.memmory.write(address, self.register_a);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;

        self.update_zero_flag(self.register_x);
        self.update_negative_flag(self.register_x);
    }

    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);

        self.update_zero_flag(self.register_x);
        self.update_negative_flag(self.register_x);
    }

    fn sbc(&mut self, mode: &AddressingMode) {
        let address = self.get_operand_address(mode);
        let value = self.memmory.read(address);

        if self.register_a < self.register_a.wrapping_sub(value) {
            self.set_carry_flag(false);
        } else {
            self.set_carry_flag(true);
        }

        self.register_a = self.register_a.wrapping_sub(value);

        self.update_zero_flag(self.register_a);
        self.update_negative_flag(self.register_a);
    }

    fn pha(&mut self) {
        self.stack.push(self.register_a);
    }

    fn php(&mut self) {
        self.stack.push(self.register_sr);
    }

    fn pla(&mut self) {
        let value = self.stack.pop().unwrap();
        self.register_a = value;

        self.update_zero_flag(self.register_a);
        self.update_negative_flag(self.register_a);
    }

    fn plp(&mut self) {
        let value = self.stack.pop().unwrap();
        self.register_sr = value;
    }

    fn update_zero_flag(&mut self, result: u8) {
        if result == 0 {
            self.register_sr |= 0b0000_0010;
        } else {
            self.register_sr &= 0b1111_1101;
        }
    }

    fn update_negative_flag(&mut self, result: u8) {
        if result & 0b1000_0000 != 0 {
            self.register_sr |= 0b1000_0000;
        } else {
            self.register_sr &= 0b0111_1111;
        }
    }

    fn set_carry_flag(&mut self, status: bool) {
        if status {
            self.register_sr |= 0b0000_0001;
        } else {
            self.register_sr &= 0b1111_1110;
        }
    }

    fn set_decimal_flag(&mut self, status: bool) {
        if status {
            self.register_sr |= 0b0001_0000;
        } else {
            self.register_sr &= 0b1110_1111;
        }
    }

    fn set_interrupt_disable_flag(&mut self, status: bool) {
        if status {
            self.register_sr |= 0b0000_1000;
        } else {
            self.register_sr &= 0b1111_0111;
        }
    }

    fn set_overflow_flag(&mut self, status: bool) {
        if status {
            self.register_sr |= 0b0100_0000;
        } else {
            self.register_sr &= 0b1011_1111;
        }
    }

    pub fn reset(&mut self) {
        self.register_a = 0x00;
        self.register_x = 0x00;
        self.register_y = 0x00;
        self.register_sr = 0b0000_0000;
        self.register_pc = 0x00;

        self.register_pc = self.memmory.read_u16(ROM_FIRST_ADDRESS);
    }
}
