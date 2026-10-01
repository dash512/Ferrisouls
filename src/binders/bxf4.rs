
pub struct BDT4Header {
    _version: [u8;4], // asserted b"BDF4"
    unk1: u8,
    unk2: u8,
    _pad1: [u8;3], // b'\0'*3

    big_endian: bool,
    bit_little_endian: bool,
    _pad2: [u8;5], // b'\0'*5

    _header_size: i64, // asserted 0x30
    signature: [u8;8], // ascii encoded
    _pad3: [u8;16] // b'\0'*16
}

//TODO