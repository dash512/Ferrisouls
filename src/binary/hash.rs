pub struct HashTableHeader {
    pad1: Vec<u8>, // b'\0' * 8
    path_hashes_offset: isize,
    hash_group_count: usize,
    _unk3: u32 // 0x00080810
}


impl HashTableHeader {
    fn new(hash_offset: isize, group_count: usize) -> Self {
        HashTableHeader {
            pad1: vec![b'\0'; 8],
            path_hashes_offset: hash_offset,
            hash_group_count: group_count,
            _unk3: 0x00080810
        }
    }
}

pub fn hash_path(path: &str) -> usize {
    /* Implementation of FromSoftware's string hashing algo.
    Always starts with a `/` which also separates path elements. */
    let path = path.replace('\\', "/");
    let mut bytes = path.into_bytes();

    if !bytes.starts_with(b"/") {
        bytes.insert(0, b'/');
    }

    let mut h = 0usize;
    for i in 0..bytes.len() {
        let chr = bytes.pop().unwrap();
        h += i * 37 + chr as usize;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_path() {
        assert_eq!(hash_path(&"path/to/your/asset"), 8178);
    }
}