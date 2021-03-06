use std::vec;

use super::cartridges::Mirroring;
use bitflags::bitflags;

pub static SYSTEM_PALLETE: [(u8, u8, u8); 64] = [
    (0x80, 0x80, 0x80),
    (0x00, 0x3D, 0xA6),
    (0x00, 0x12, 0xB0),
    (0x44, 0x00, 0x96),
    (0xA1, 0x00, 0x5E),
    (0xC7, 0x00, 0x28),
    (0xBA, 0x06, 0x00),
    (0x8C, 0x17, 0x00),
    (0x5C, 0x2F, 0x00),
    (0x10, 0x45, 0x00),
    (0x05, 0x4A, 0x00),
    (0x00, 0x47, 0x2E),
    (0x00, 0x41, 0x66),
    (0x00, 0x00, 0x00),
    (0x05, 0x05, 0x05),
    (0x05, 0x05, 0x05),
    (0xC7, 0xC7, 0xC7),
    (0x00, 0x77, 0xFF),
    (0x21, 0x55, 0xFF),
    (0x82, 0x37, 0xFA),
    (0xEB, 0x2F, 0xB5),
    (0xFF, 0x29, 0x50),
    (0xFF, 0x22, 0x00),
    (0xD6, 0x32, 0x00),
    (0xC4, 0x62, 0x00),
    (0x35, 0x80, 0x00),
    (0x05, 0x8F, 0x00),
    (0x00, 0x8A, 0x55),
    (0x00, 0x99, 0xCC),
    (0x21, 0x21, 0x21),
    (0x09, 0x09, 0x09),
    (0x09, 0x09, 0x09),
    (0xFF, 0xFF, 0xFF),
    (0x0F, 0xD7, 0xFF),
    (0x69, 0xA2, 0xFF),
    (0xD4, 0x80, 0xFF),
    (0xFF, 0x45, 0xF3),
    (0xFF, 0x61, 0x8B),
    (0xFF, 0x88, 0x33),
    (0xFF, 0x9C, 0x12),
    (0xFA, 0xBC, 0x20),
    (0x9F, 0xE3, 0x0E),
    (0x2B, 0xF0, 0x35),
    (0x0C, 0xF0, 0xA4),
    (0x05, 0xFB, 0xFF),
    (0x5E, 0x5E, 0x5E),
    (0x0D, 0x0D, 0x0D),
    (0x0D, 0x0D, 0x0D),
    (0xFF, 0xFF, 0xFF),
    (0xA6, 0xFC, 0xFF),
    (0xB3, 0xEC, 0xFF),
    (0xDA, 0xAB, 0xEB),
    (0xFF, 0xA8, 0xF9),
    (0xFF, 0xAB, 0xB3),
    (0xFF, 0xD2, 0xB0),
    (0xFF, 0xEF, 0xA6),
    (0xFF, 0xF7, 0x9C),
    (0xD7, 0xE8, 0x95),
    (0xA6, 0xED, 0xAF),
    (0xA2, 0xF2, 0xDA),
    (0x99, 0xFF, 0xFC),
    (0xDD, 0xDD, 0xDD),
    (0x11, 0x11, 0x11),
    (0x11, 0x11, 0x11),
];

bitflags! {

   // 7  bit  0
   // ---- ----
   // VPHB SINN
   // |||| ||||
   // |||| ||++- Base nametable address
   // |||| ||    (0 = $2000; 1 = $2400; 2 = $2800; 3 = $2C00)
   // |||| |+--- VRAM address increment per CPU read/write of PPUDATA
   // |||| |     (0: add 1, going across; 1: add 32, going down)
   // |||| +---- Sprite pattern table address for 8x8 sprites
   // ||||       (0: $0000; 1: $1000; ignored in 8x16 mode)
   // |||+------ Background pattern table address (0: $0000; 1: $1000)
   // ||+------- Sprite size (0: 8x8 pixels; 1: 8x16 pixels)
   // |+-------- PPU master/slave select
   // |          (0: read backdrop from EXT pins; 1: output color on EXT pins)
   // +--------- Generate an NMI at the start of the
   //            vertical blanking interval (0: off; 1: on)

   struct ControlRegister: u8 {
       const NAMETABLE1                = 0b00000001;
       const NAMETABLE2                = 0b00000010;
       const VRAM_ADD_INCREMENT        = 0b00000100;
       const SPRITE_PATTERN_ADDRESS    = 0b00001000;
       const BACKROUND_PATTERN_ADDRESS = 0b00010000;
       const SPRITE_SIZE               = 0b00100000;
       const MASTER_SLAVE_SELECT       = 0b01000000;
       const GENERATE_NMI              = 0b10000000;
   }
}

impl ControlRegister {
    fn new() -> Self {
        ControlRegister::from_bits_truncate(0b00000000)
    }

    fn vram_address_increment(&self) -> u8 {
        if self.contains(ControlRegister::VRAM_ADD_INCREMENT) {
            32
        } else {
            1
        }
    }

    fn update(&mut self, value: u8) {
        self.bits = value;
    }

    fn generate_vblank_nmi(&self) -> bool {
        self.contains(ControlRegister::GENERATE_NMI)
    }
}

struct AddressRegister {
    low: u8,
    high: u8,
    high_pointer: bool,
}

impl AddressRegister {
    fn new() -> Self {
        Self {
            low: 0,
            high: 0,
            high_pointer: true,
        }
    }

    fn get(&self) -> u16 {
        let high_u16 = self.high as u16;
        let low_u16 = self.low as u16;

        (high_u16 << 8) | low_u16
    }

    fn set(&mut self, value: u16) {
        self.high = (value >> 8) as u8;
        self.low = (value & 0xff) as u8;
    }

    fn update(&mut self, value: u8) {
        if self.high_pointer {
            self.high = value;
        } else {
            self.low = value;
        }

        if self.get() > 0x3fff {
            self.set(self.get() & 0b11111111111111);
        }

        self.high_pointer = self.high_pointer;
    }

    fn increment(&mut self, value: u8) {
        let old_low = self.low;
        self.low = self.low.wrapping_add(value);

        if old_low > self.low {
            self.high = self.high.wrapping_add(1);
        }

        if self.get() > 0x3fff {
            self.set(self.get() & 0b11111111111111);
        }
    }

    fn reset_latch(&mut self) {
        self.high_pointer = true;
    }
}

bitflags! {

    // 7  bit  0
    // ---- ----
    // VSO. ....
    // |||| ||||
    // |||+-++++- Least significant bits previously written into a PPU register
    // |||        (due to register not being updated for this address)
    // ||+------- Sprite overflow. The intent was for this flag to be set
    // ||         whenever more than eight sprites appear on a scanline, but a
    // ||         hardware bug causes the actual behavior to be more complicated
    // ||         and generate false positives as well as false negatives; see
    // ||         PPU sprite evaluation. This flag is set during sprite
    // ||         evaluation and cleared at dot 1 (the second dot) of the
    // ||         pre-render line.
    // |+-------- Sprite 0 Hit.  Set when a nonzero pixel of sprite 0 overlaps
    // |          a nonzero background pixel; cleared at dot 1 of the pre-render
    // |          line.  Used for raster timing.
    // +--------- Vertical blank has started (0: not in vblank; 1: in vblank).
    //            Set at dot 1 of line 241 (the line *after* the post-render
    //            line); cleared after reading $2002 and at dot 1 of the
    //            pre-render line.

    struct StatusRegister: u8 {
        const NOT_USED          = 0b0000_0001;
        const NOT_USED2         = 0b0000_0010;
        const NOT_USED3         = 0b0000_0100;
        const NOT_USED4         = 0b0000_1000;
        const NOT_USED5         = 0b0001_0000;
        const SPRITE_OVERFLOW   = 0b0010_0000;
        const SPRITE_ZERO_HIT   = 0b0100_0000;
        const VBLANK_STARTED    = 0b1000_0000;
    }
}

impl StatusRegister {
    fn new() -> Self {
        StatusRegister::from_bits_truncate(0b0000_0000)
    }

    fn set_vblank_status(&mut self, status: bool) {
        self.set(StatusRegister::VBLANK_STARTED, status);
    }

    fn reset_vblank_status(&mut self) {
        self.remove(StatusRegister::VBLANK_STARTED);
    }

    fn is_in_vblank(&self) -> bool {
        self.contains(StatusRegister::VBLANK_STARTED)
    }
}

pub struct PPU {
    pub chr_rom: Vec<u8>,
    pub pallete_table: [u8; 32],
    pub vram: [u8; 2048],
    pub oam_data: [u8; 256],
    pub mirroring: Mirroring,
    address: AddressRegister,
    control: ControlRegister,
    status: StatusRegister,
    internal_data_buffer: u8,
    scanline: u16,
    cycles: usize,
    nmi_interrupt: Option<u8>,
}

impl PPU {
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        Self {
            chr_rom,
            pallete_table: [0; 32],
            vram: [0; 2048],
            oam_data: [0; 64 * 4],
            mirroring,
            address: AddressRegister::new(),
            control: ControlRegister::new(),
            status: StatusRegister::new(),
            internal_data_buffer: 0,
            scanline: 0,
            cycles: 0,
            nmi_interrupt: None,
        }
    }

    pub fn write_in_ppu_address(&mut self, value: u8) {
        self.address.update(value);
    }

    pub fn write_in_control(&mut self, value: u8) {
        let before_nmi_status = self.control.generate_vblank_nmi();
        self.control.update(value);

        if !before_nmi_status && self.control.generate_vblank_nmi() && self.status.is_in_vblank() {
            self.nmi_interrupt = Some(1);
        }
    }

    fn increment_vram_address(&mut self) {
        self.address
            .increment(self.control.vram_address_increment());
    }

    fn mirror_vram_address(&self, address: u16) -> u16 {
        let mirrored_address = address & 0b10111111111111;
        let vram_index = mirrored_address - 0x2000;
        let name_table = vram_index / 0x400;

        match (&self.mirroring, name_table) {
            (Mirroring::Vertical, 2) | (Mirroring::Vertical, 3) => vram_index - 0x800,
            (Mirroring::Horizontal, 2) => vram_index - 0x400,
            (Mirroring::Horizontal, 1) => vram_index - 0x400,
            (Mirroring::Horizontal, 3) => vram_index - 0x800,
            _ => vram_index,
        }
    }

    pub fn read_data(&mut self) -> u8 {
        let address = self.address.get();
        self.increment_vram_address();

        match address {
            0..=0x1fff => {
                let result = self.internal_data_buffer;
                self.internal_data_buffer = self.chr_rom[address as usize];
                result
            }

            0x2000..=0x2fff => {
                let result = self.internal_data_buffer;
                self.internal_data_buffer = self.vram[self.mirror_vram_address(address) as usize];
                result
            }

            0x3000..=0x3eff => panic!("Read unexpected address: 0x{:02x}", address),
            0x3f00..=0x3fff => self.pallete_table[(address - 0x3f00) as usize],
            _ => panic!("Read unexpected address: 0x{:02x}", address),
        }
    }

    pub fn write_in_data(&mut self, data: u8) {
        let address = self.address.get();

        match address {
            0..=0x1FFF => println!("Attempt to write to chr_rom space: 0x{:02x}", address),
            0x2000..=0x2FFF => self.vram[self.mirror_vram_address(address) as usize] = data,
            0x3000..=0x3EFF => unimplemented!("Address {} should't be used in reallity", address),

            0x3F10 | 0x3F14 | 0x3F18 | 0x3F1C => {
                let add_mirror = address - 0x10;
                self.pallete_table[(add_mirror - 0x3F00) as usize] = data;
            }

            0x3f00..=0x3fff => {
                self.pallete_table[(address - 0x3f00) as usize] = data;
            }

            _ => panic!("Unexpected access to mirrored space {}", address),
        }
        self.increment_vram_address();
    }

    pub fn tick(&mut self, cycles: u8) -> bool {
        self.cycles += cycles as usize;

        if !self.cycles >= 341 {
            return false;
        }

        self.cycles -= 341;
        self.scanline += 1;

        if self.scanline == 241 {
            self.status.set_vblank_status(true);
            if self.control.generate_vblank_nmi() {
                self.nmi_interrupt = Some(1);
            }
        }

        if self.scanline >= 262 {
            self.scanline = 0;
            self.nmi_interrupt = None;
            self.status.reset_vblank_status();
            return true;
        }

        return false;
    }

    pub fn pool_nmi_status(&mut self) -> Option<u8> {
        self.nmi_interrupt.take()
    }
}

pub struct Frame {
    data: Vec<u8>,
}

impl Frame {
    const WIDTH: usize = 256;
    const HEIGHT: usize = 240;

    pub fn new() -> Self {
        Frame {
            data: vec![0; (Frame::WIDTH) * (Frame::HEIGHT) * 3],
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, rgb: (u8, u8, u8)) {
        let base = y * 3 * Frame::WIDTH + x * 3;

        if base + 2 < self.data.len() {
            self.data[base] = rgb.0;
            self.data[base + 1] = rgb.1;
            self.data[base + 2] = rgb.2;
        }
    }

    pub fn get_data(&self) -> &Vec<u8> {
        &self.data
    }
}
