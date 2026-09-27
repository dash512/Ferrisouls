use hex_literal::hex;
use crate::common::dcx::DCXType;


pub struct Game {
    pub name: &'static str,
    pub regulation_key: Option<[u8;32]>,
    pub sl2_key: Option<[u8;16]>,
    pub default_dcx_type: DCXType


}
impl Game {
    pub const DES: Game = Game {
        name: "Demon's Souls",
        regulation_key: None,
        sl2_key: None,
        default_dcx_type: DCXType::DCX_EDGE
    };

    pub const PTDE: Game = Game {
        name: "Dark Souls 1: Prepare to Die Edition",
        regulation_key: None,
        sl2_key: None,
        default_dcx_type: DCXType::Null // no dcx used
    };

    pub const DSR: Game = Game {
        name: "Dark Souls: Remastered",
        regulation_key: None,
        sl2_key: Some(hex!("01 23 45 67 89 AB CD EF FE DC BA 98 76 54 32 10")),
        default_dcx_type: DCXType::DCX_DFLT_10000_24_9
    };

    pub const DS2: Game = Game {
        name: "Dark Souls 2",
        regulation_key: None,
        sl2_key: Some(hex!("B7 FD 46 3E 4A 9C 11 02 DF 17 39 E5 F3 B2 A5 0F")),
        default_dcx_type: DCXType::DCX_DFLT_10000_24_9 // Null for icons as tpfs aren't compressed
    };

    pub const SOTFS: Game = Game {
        name: "Dark Souls 2: Scholar of the First Sin",
        regulation_key: None,
        sl2_key: Some(hex!("59 9F 9B 69 96 40 A5 52 36 EE 2D 70 83 5E C7 44")),
        default_dcx_type: DCXType::DCX_DFLT_10000_24_9 // Null for icons as tpfs aren't compressed
    };

    pub const BB: Game = Game {
        name: "Bloodborne",
        regulation_key: None,
        sl2_key: None,
        default_dcx_type: DCXType::DCX_DFLT_10000_44_9
    };

    pub const DS3: Game = Game {
        name: "Dark Souls 3",
        regulation_key: Some(hex!("64 73 33 23 6A 6E 2F 38 5F 37 28 72 73 59 39 70 67 35 35 47 46 4E 37 56 46 4C 23 2B 33 6E 2F 29")),
        sl2_key: Some(hex!("FD 46 4D 69 5E 69 A3 9A 10 E3 19 A7 AC E8 B7 FA")),
        default_dcx_type: DCXType::DCX_DFLT_10000_44_9
    };

    pub const SDT: Game = Game {
        name: "Sekiro: Shadows Die Twice",
        regulation_key: None,
        sl2_key: None,
        default_dcx_type: DCXType::DCX_KRAK
    };

    pub const ER: Game = Game {
        name: "Elden Ring",
        regulation_key: Some(hex!("99 BF FC 36 6A 6B C8 C6 F5 82 7D 09 36 02 D6 76 C4 28 92 A0 1C 20 7F B0 24 D3 AF 4E 49 3F EF 99")),
        sl2_key: None,
        default_dcx_type: DCXType::DCX_KRAK // Null for reg.bin
    };

    pub const AC6: Game = Game {
        name: "Armored Core 6",
        regulation_key: Some(hex!("10 CE ED 47 7B 7C D9 D7 E6 93 8E 11 47 13 E7 87 D5 39 13 B1 0D 31 8E C1 35 E4 BE 50 50 4E 0E 10")),
        sl2_key: Some(hex!("B1 56 87 9F 13 48 97 98 70 05 C4 87 00 AE F8 79")),
        default_dcx_type: DCXType::DCX_KRAK
    };

    pub const NR: Game = Game {
        name: "Elden Ring: Nightreign",
        regulation_key: Some(hex!("9A 8E E9 0C 4C 01 A4 31 68 A1 7D 9D 75 E4 A7 D0 21 07 EB CF 43 D5 AC B0 55 4F 94 16 01 B5 79 18")),
        sl2_key: Some(hex!("18 F6 32 66 05 BD 17 8A 55 24 52 3A C0 A0 C6 09")),
        default_dcx_type: DCXType::DCX_KRAK
    };
    
}


