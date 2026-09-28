use bitflags::bitflags;

use crate::games::Game;


pub enum BinderVersion {
    V3,//all games pre DS2 (2014)
    V4//all games after DS2 (2014)
}



bitflags! {
    pub struct BinderFlags: u32 {
        const IS_BIG_ENDIAN = 0b0000_0001;
        const HAS_IDS = 0b0000_0010;
        const HAS_NAMES_1 = 0b0000_0100;
        const HAS_NAMES_2 = 0b0000_1000;
        const HAS_NAMES = 0b0000_1100; // checks both. Difference unknown.
        const HAS_LONG_OFFSETS = 0b0001_0000;
        const HAS_COMPRESSION = 0b0010_0000;
        const FLAG_6 = 0b0100_0000;
        const FLAG_7 = 0b1000_0000;

        const _ = !0;
    }

    pub struct EntryFlags: u32 {
        const NONE = 0;
        const COMPRESSED = 0b0000_0001;
        const FLAG_1 = 0b0000_0010;
        const FLAG_2 = 0b0000_0100;
        const FLAG_3 = 0b0000_1000;
        const FLAG_4 = 0b0001_0000; // checks both. Difference unknown.
        const FLAG_5 = 0b0010_0000;
        const FLAG_6 = 0b0100_0000;
        const FLAG_7 = 0b1000_0000;

        const _ = !0;
    }

}

impl BinderFlags {
    pub fn has_names(&self) -> bool {
        self.intersects(Self::HAS_NAMES_1 | Self::HAS_NAMES_2)
    }

    pub fn get_bnd_entry_header_size(self) -> usize {
        let mut size = 16;

        if self.contains(Self::HAS_IDS) {
            size += 4;
        }

        if self.has_names() {
            size += 4;
        }

        if self.contains(Self::HAS_COMPRESSION) {
            size += 8;
        }

        size += if self.contains(Self::HAS_LONG_OFFSETS) {
            8
        } else {
            4
        };

        size
    }

    fn reverse_bits(byte: u8) -> u8 {
        byte.reverse_bits()
    }

    pub fn from_byte(byte: u8, big_endian: bool) -> Self {
        let mut value = byte as u32;

        let flags = Self::from_bits_truncate(value);

        if !big_endian
            && !(flags.contains(Self::IS_BIG_ENDIAN)
                && !flags.contains(Self::FLAG_7))
        {
            value = value.reverse_bits();
        }

        Self::from_bits_truncate(value)
    }

    pub fn to_byte(self, big_endian: bool) -> u8 {
        let value = self.bits();

        if !big_endian
            && !(self.contains(Self::IS_BIG_ENDIAN)
                && !self.contains(Self::FLAG_7))
        {
            value.reverse_bits().try_into().unwrap()
        } else {
            value.try_into().unwrap()
        }
    }
}

impl EntryFlags {
    
    fn reverse_bits(byte: u8) -> u8 {
        byte.reverse_bits()
    }

    pub fn from_byte(byte: u8, big_endian: bool) -> Self {
        let mut value = byte as u32;

        let flags = Self::from_bits_truncate(value);

        if !big_endian {
            value = value.reverse_bits();
        }

        Self::from_bits_truncate(value)
    }

    pub fn to_byte(self, big_endian: bool) -> u8 {
        let value = self.bits();

        if !big_endian {
            value.reverse_bits().try_into().unwrap()
        } else {
            value.try_into().unwrap()
        }
    }

}



pub struct BND3Header {
    version: [u8;4], // asserted [b"BND3", b"BHF3"]
    signature: [u8;8], // ascii encoded
    flags: u8,
    big_endian: bool,
    bit_big_endian: bool,
    _pad1: u8, // b'\0'
    entry_count: usize,
    file_size: usize,
    _pad2: [u8;8] // b'\0'*8
}

pub struct BND4Header {
    version: [u8;4], // asserted [b"BND4", b"BHF4"]
    unknown1: bool,
    unknown2: bool,
    _pad1: [u8;3], // b'\0'*3
    big_endian: bool,
    bit_little_endian: bool,
    _pad2: u8, // b'\0'
    entry_count: usize,
    // NOTE: No `file_size` in V4.
    _header_size: i64, // asserted 0x40
    signature: [u8;8], // ascii encoded
    _entry_header_size: i64,
    _data_offset: i64,
    unicode: bool,
    flags: u8,
    hash_table_type: u8, // asserted [0, 1, 4, 128]
    _pad3: [u8;5], // b'\0'*5
    _hash_table_offset: i64 // only non-zero if `hash_table_type = 4` - Grimrukh
}


pub struct BDT3Header {
    _version: [u8;4], // asserted b"BDF3"
    signature: [u8;8], // ascii encoded
    _pad1: [u8;4] // b'\0'*4
}


pub struct BDT4Header {
    _version: [u8;4], // asserted b"BDF4"
    unknown1: bool,
    unknown2: bool,
    _pad1: [u8;3], // b'\0'*3
    big_endian: bool,
    bit_little_endian: bool,
    _pad2: [u8;5], // b'\0'*5
    _header_size: i64, // asserted 0x30
    signature: [u8;8], // ascii encoded
    _pad3: [u8;16] // b'\0'*16
}


pub struct BND4Info {
    unk1: bool,
    unk2: bool,
    unk3: bool,
    hash_table_type: i32
}   

impl BND4Info {
    pub fn new(unk1: bool, unk2: bool, unk3: bool, hash_table_type: i32) -> Self {
        Self { unk1, unk2, unk3, hash_table_type }
    }

    pub fn game_default(game: Game) -> Self {
        let table_type = match game {
            Game::BB => 0,
            //value hasn't changed since DS3
            Game::DS3 => 4,
            Game::SDT => 4,
            Game::ER => 4,
            Game::AC6 => 4,
            Game::NR => 4,
            _=> 0 // TODO: is this correct for sotfs?
        };
        Self { unk1: false, unk2: false, unk3: true, hash_table_type: table_type }
    }
}



pub struct Binder {
    signature: [u8;8], // ascii encoded; default '07D7R6'
    flags: BinderFlags, // most commonly: 0b00101110
    big_endian: bool,
    bit_big_endian: bool,
    version: BinderVersion,
    v4_info: Option<BND4Info>,

    is_split_bxf: bool,

    entries: Vec<BinderEntry>
}

impl Binder {
    
}



pub struct BinderEntryHeader {
    flags: EntryFlags,
    compressed_size: usize,
    entry_id: Option<usize>,
    path:  Option<String>,
    uncompressed_size: Option<usize>,
    data_offset: i64
}

impl BinderEntryHeader {
    
}

pub struct BinderEntry {
    data: Vec<u8>,
    entry_id: usize, // internal index
    path: Option<String>, // UTF-16/Shift-JIS with double backslashes
    flags: EntryFlags, // default 0x2; compression.
}

impl BinderEntry {
    
}


