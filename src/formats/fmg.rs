use std::collections::HashMap;

use crate::{binary::{BinaryReader, BinaryWriter, IO}, errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError}};



#[derive(Debug, PartialEq)]
pub enum FMGVersion {
    DES = 0, //DeS
    DS1 = 1, // DS1/2
    DS3 = 2 //DS3/BB
}

impl FMGVersion {
    pub fn from_byte(byte: u8) -> Result<Self, BinaryReaderError> {
        match byte {
            0=> Ok(Self::DES),
            1=> Ok(Self::DS1),
            2=> Ok(Self::DS3),
            _=> Err(BinaryReaderError::InvalidData(
                format!("Invalid FMG version read: {}", byte)
            ))
        }
    }

    pub fn to_byte(&self) -> Result<u8, BinaryWriterError> {
        match self {
            Self::DES => Ok(0u8),
            Self::DS1 => Ok(1u8),
            Self::DS3 => Ok(2u8),
            _=> Err(BinaryWriterError::InvalidData(
                format!("Invalid FMG version read: {:?}", self)
            ))
        }
    }
}

pub struct FMGEntry {
    pub id: usize,
    pub text: String
}

pub struct FMG {
    version: FMGVersion, 

    big_endian: bool,
    unicode: bool,

    hash: bool, // if true, add MD5 hash to the top of the file
    reuse_offsets: bool, // to save space on duplicate entries

    entries: Vec<FMGEntry>
}

impl FMG {
    pub fn new(version: FMGVersion, hash: bool, reuse_offsets: bool) -> Self {
        Self {
            big_endian: (version==FMGVersion::DES),
            version,
            unicode: true,
            hash,
            reuse_offsets,
            entries: Vec::new()
        }
    }

    pub fn sort_entries(&mut self) {
        self.entries.sort_by(|a, b| a.id.cmp(&b.id));
    }

    //write strings to offset, reusing them for ids that have duplicate text if `reuse_offsets` is true.
    fn write_strings(&mut self, writer: &mut BinaryWriter, reuse_offsets: bool) -> Result<(), BinaryWriterError> {
        if !reuse_offsets {
            for (idx,e) in self.entries.iter().enumerate() {
                if e.text.len() > 0 {
                    writer.fill_varint(format!("StringOffset{}", idx), writer.position() as i64)?;

                    if self.unicode {
                        writer.write_utf16(&e.text, true)?;
                    } else {
                        writer.write_shift_jis(&e.text, true)?;
                    }
                } else {
                    writer.fill_varint(format!("StringOffset{}", idx), 0i64)?;
                }
            }
            Ok(())
        }

        else {
            let mut offsets: HashMap<String, i64> = HashMap::new();

            for (idx,e) in self.entries.iter().enumerate() {
                if e.text.len() > 0 {
                    match offsets.get(&e.text) {
                        Some(offset) => writer.fill_varint(format!("StringOffset{}", idx), *offset)?,
                        None => {
                            let offset = writer.position() as i64;
                            offsets.insert(e.text.clone(), offset);
                            writer.fill_varint(format!("StringOffset{}", idx), offset)?;

                            if self.unicode {
                                writer.write_utf16(&e.text, true)?;
                            } else {
                                writer.write_shift_jis(&e.text, true)?;
                            }
                        }
                    }
                }
                
                else {
                    writer.fill_varint(format!("StringOffset{}", idx), 0i64)?;
                }
            }
            Ok(())
        }
    }

}

impl FMG {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let is_hash = reader.read_u8()? != 0u8;
        if is_hash {
            reader.set_position(reader.position()+16);
        }

        reader.assert::<u8>(0u8)?;

        let big_endian = reader.read_boolean()?;
        reader.big_endian = big_endian;
        let version = FMGVersion::from_byte(reader.read_u8()?)?;

        reader.assert::<u8>(0u8)?;

        let long = if version == FMGVersion::DS3 {
            reader.varint_long = true;
            true
        } else {
            false
        };

        let file_size = reader.read_i32()?;
        let unicode = reader.read_boolean()?;

        if version == FMGVersion::DES {
            reader.assert::<u8>(0xFFu8)?;
        } else {
            reader.assert::<u8>(0x00u8)?;
        };

        reader.assert::<u8>(0u8)?;
        reader.assert::<u8>(0u8)?;

        let group_count = reader.read_i32()?;
        reader.read_i32()?;//string count

        if long {
            reader.assert::<i32>(0xFFi32)?;
        };

        let mut string_offsets_loc = reader.read_varint()?;
        if is_hash {
            string_offsets_loc += 16
        };

        reader.assert_varint(0)?;

        let mut entries = Vec::new();

        for i in 0..group_count {
            let offset = reader.read_i32()? as i64;
            let first = reader.read_i32()?;
            let last = reader.read_i32()?;

            let oc = if long {
                reader.assert::<i32>(0i32)?;
                8i64
            } else {
                4i64
            };

            let pos = string_offsets_loc + offset * oc;
            reader.step_in(pos.try_into().unwrap())?;

            for j in 0..last-first+1 {
                let mut string_offset = reader.read_varint()?;
                if is_hash {
                    string_offset += 16;
                }

                let id = (first + j) as usize;
                let text = if string_offset > 0 {
                    if unicode {
                        reader.step_in(string_offset as u64)?;
                        let text = reader.read_utf16()?;
                        reader.step_out()?;
                        text
                    } else {
                        reader.step_in(string_offset as u64)?;
                        let text = reader.read_shift_jis()?;
                        reader.step_out()?;
                        text
                    }
                } else {
                    String::new()
                };

                entries.push(FMGEntry { id, text } );
            }

            reader.step_out()?;
        }

        Ok(
            Self {
                version,
                big_endian,
                unicode,
                hash: is_hash,
                reuse_offsets: false,
                entries
            }
        )
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        let long = (self.version == FMGVersion::DS3);

        writer.big_endian = self.big_endian;
        writer.varint_long = long;

        writer.pad(1)?;
        writer.write_boolean(self.big_endian)?;
        writer.write_u8(self.version.to_byte()?)?;
        writer.pad(1)?;

        writer.reserve::<i32>("FileSize".to_string())?;

        writer.write_boolean(self.unicode)?;
        if self.version == FMGVersion::DES {
            writer.write_u8(0xFFu8)?;
        } else {
            writer.write_u8(0x00u8)?;
        }
        writer.pad(2)?; //2 separate 0u8s in SFNext

        writer.reserve::<i32>("GroupCount".to_string())?;
        writer.write_i32(self.entries.len().try_into().unwrap())?;

        if long {
            writer.write_i32(0xFFi32)?;
        }

        writer.reserve_varint("StringOffsets".to_string())?;
        writer.write_varint(0i64)?;

        let mut group_count = 0;
        self.sort_entries();
        for (mut idx,e) in self.entries.iter().enumerate() {
            writer.write_i32(idx as i32)?;
            writer.write_i32(e.id as i32)?;

            while (idx < self.entries.len() - 1) 
                && self.entries[idx+1].id
                   == self.entries[idx].id + 1
                {idx+=1;}

            writer.write_i32(self.entries.get(idx).unwrap().id as i32)?;

            if long {
                writer.write_i32(0i32)?;
            }

            group_count+=1;
        }

        writer.fill("GroupCount".to_string(), group_count)?;
        writer.fill("StringOffsets".to_string(), writer.position() as i64)?;
        
        for i in 0..self.entries.len() {
            writer.reserve_varint(format!("StringOffset{i}"))?;
        }

        self.write_strings(writer, self.reuse_offsets);

        writer.fill("FileSize".to_string(), writer.position() as i32)?;

        if self.hash {
            writer.prepend_md5_hash()?;
        }

        Ok(())
    }

}

