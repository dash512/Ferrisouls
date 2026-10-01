
pub struct BDT3Header {
    _version: [u8;4], // asserted b"BDF3"
    signature: [u8;8], // ascii encoded
    _pad1: [u8;4] // b'\0'*4
}

//TODO