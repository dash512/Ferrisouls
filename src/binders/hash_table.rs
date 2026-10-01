use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError};
use crate::binary::{BinaryReader, BinaryWriter, IO};

use primes::is_prime;

///Unused in I/O
pub struct HashTableHeader {
    hashes_offset: u64,
    group_count: u32,
    _unk3: u32 // 0x00080810
}



#[derive(Debug, Clone)]
pub struct BinderHashTable {
    pub groups: Vec<BinderHashGroup>,
    pub hashes: Vec<BinderPathHash>,
}

#[derive(Debug, Clone, Copy)]
pub struct BinderHashGroup {
    pub index: i32,
    pub length: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct BinderPathHash {
    pub hash: u32,
    pub entry_index: i32,
}

impl BinderHashTable {
    fn validate(&self) -> Result<(), FerrisoulsError> {
        let mut expected_index = 0i32;

        for group in &self.groups {
            if group.index != expected_index {
                return Err(BinaryWriterError::Custom(
                    format!(
                        "Invalid hash group index: expected {}, got {}",
                        expected_index, group.index
                    )
                ).into());
            }

            if group.length < 0 {
                return Err(BinaryWriterError::Custom(
                    "Hash group has negative length.".to_string()
                ).into());
            }

            expected_index += group.length;
        }

        if expected_index as usize != self.hashes.len() {
            return Err(BinaryWriterError::Custom(
                format!(
                    "Hash group lengths total {}, but there are {} hashes.",
                    expected_index,
                    self.hashes.len()
                )
            ).into());
        }

        Ok(())
    }

}

impl IO for BinderHashTable {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, FerrisoulsError> {
        let hashes_offset = reader.read_u64()?;

        let group_count = reader.read_u32()? as usize;

        reader.assert_bytes(&[0x10, 0x08, 0x08, 0x00])?;

        let mut groups = Vec::with_capacity(group_count);

        for _ in 0..group_count {
            let length = reader.read_i32()?;
            let index = reader.read_i32()?;

            groups.push(BinderHashGroup {index, length});
        }

        reader.step_in(hashes_offset)?;

        let hash_count: usize = groups
            .iter()
            .map(|g| {
                if g.length < 0 {
                    return Err(BinaryReaderError::Custom(
                        format!("Invalid negative hash-group length: {}", g.length)
                    ));
                }
                Ok(g.length as usize)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum();


        let mut hashes = Vec::with_capacity(hash_count);

        for _ in 0..hash_count {
            hashes.push(BinderPathHash {hash: reader.read_u32()?, entry_index: reader.read_i32()?});
        }

        reader.step_out()?;

        Ok(Self {
            groups,
            hashes,
        })
    }

    fn to_writer(&self, writer: &mut BinaryWriter) -> Result<(), FerrisoulsError> {
        self.validate()?;
        
        writer.reserve("HashesOffset".to_string(), 8)?;

        writer.write_u32(self.groups.len() as u32)?;

        writer.write_u8(0x10)?;
        writer.write_u8(0x08)?;
        writer.write_u8(0x08)?;
        writer.write_u8(0x00)?;

        for group in &self.groups {
            writer.write_i32(group.length)?;
            writer.write_i32(group.index)?;
        }

        writer.fill::<u64>("HashesOffset".to_string(), writer.position())?;

        for path_hash in &self.hashes {
            writer.write_u32(path_hash.hash)?;
            writer.write_i32(path_hash.entry_index)?;
        }

        Ok(())
    }
    
}

impl BinderHashTable {
    pub fn from_names(names: &[String]) -> Result<Self, FerrisoulsError> {
        let group_count = find_hash_group_count(names.len())?;

        let mut buckets: Vec<Vec<BinderPathHash>> = (0..group_count)
            .map(|_| Vec::new())
            .collect();

        for (index, name) in names.iter().enumerate() {
            let hash = hash_path(name);

            let group = (hash % group_count as u32) as usize;

            buckets[group].push(BinderPathHash {
                hash: hash as u32,
                entry_index: index as i32,
            });
        }

        for bucket in &mut buckets {
            bucket.sort_by_key(|entry| entry.hash);
        }

        let mut groups = Vec::with_capacity(group_count);
        let mut hashes = Vec::new();

        let mut count = 0i32;

        for bucket in buckets {
            let index = count;

            for path_hash in bucket {
                hashes.push(path_hash);
                count += 1;
            }

            groups.push(BinderHashGroup {
                index,
                length: count - index,
            });
        }

        Ok(Self {
            groups,
            hashes,
        })
    }

}


fn find_hash_group_count(file_count: usize) -> Result<usize, FerrisoulsError> {
    let start = file_count / 7;

    for p in start..=100_000 {
        if is_prime(p.try_into().unwrap()) {
            return Ok(p);
        }
    }

    Err(BinaryWriterError::custom(
        "Could not determine hash group count.".to_string()
    ).into())
}



pub fn hash_path(path: &str) -> u32 {
    /* Implementation of FromSoftware's string hashing algo.
    Always starts with a `/` which also separates path elements. */
    let path = path.replace('\\', "/");
    let mut bytes = path.into_bytes();

    if !bytes.starts_with(b"/") {
        bytes.insert(0, b'/');
    }

    let mut hash = 0u32;
    for i in 0..bytes.len() {
        let chr = bytes.pop().unwrap() as u32;
        hash = hash.wrapping_add((i as u32).wrapping_mul(37));
        hash = hash.wrapping_add(chr);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_path() {
        assert_eq!(hash_path(&"path/to/your/asset"), 8178);
    }
}