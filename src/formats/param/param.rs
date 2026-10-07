use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;
use bitflags::bitflags;

use crate::binary::{BinaryReader, BinaryWriter, IO, decompress_if_needed};
use crate::binders::{Binder, BinderEntry, BinderVersion, MetaBinder, MetaEntry};
use crate::binders::bnd4::{BND4, BND4Entry, BND4EntryHeader, BND4Header};
use crate::dcx::{Compress, CompressSettings, DCXType};
use crate::errors::{BinaryReaderError, BinaryWriterError};
use crate::errors::FerrisoulsError;
use crate::formats::param::row::{self, Row};
use crate::formats::param::paramdef::Paramdef;
use crate::formats::param::ParamdefMatchOptions;

bitflags! {
    /// First set of flags indicating file format; highly speculative.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct FormatFlags1: u8 {
        const FLAG01 = 0b0000_0001; // unknown
        const INT_DATA_OFFSET = 0b0000_0010; // expanded header; 32 bit data offset
        const LONG_DATA_OFFSET = 0b0000_0100; // expanded header; 64 bit data offset
        const FLAG08 = 0b0000_1000;
        const FLAG10 = 0b0001_0000;
        const FLAG20 = 0b0010_0000;
        const FLAG40 = 0b0100_0000;
        const OFFSET_PARAM_TYPE = 0b1000_0000; // param-type string is written separately; not fixed-width in header
    }
}

bitflags! {
    /// Second set of flags indicating file format; highly speculative.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct FormatFlags2: u8 {
        const UNICODE_ROW_NAMES = 0b0000_0001; // row names are utf-16
        const FLAG02 = 0b0000_0010;
        const FLAG04 = 0b0000_0100;
        const FLAG08 = 0b0000_1000;
        const FLAG10 = 0b0001_0000;
        const FLAG20 = 0b0010_0000;
        const FLAG40 = 0b0100_0000;
        const FLAG80 = 0b1000_0000;
    }
}

///A general-purpose configuration file used throughout the series.
#[derive(Debug, Clone, Default)]
pub struct Param {
    pub name: Option<String>,
    pub header: Option<BND4EntryHeader>,

    pub big_endian: bool, // only true for PS3 and xbox 360
    pub format_2d: FormatFlags1,
    pub format_2e: FormatFlags2,
    pub paramdef_format_version: u8, // 0 or 0xFF. For old version 101 it matched the paramdef
    pub unk06: i16,

    pub paramdef_data_version: i16, // indicates row data structure rivision
    pub param_type: String,
    pub detected_size: i64, // found from row offset spacing. -1 if param has no rows.
    
    pub applied_paramdef: Option<Paramdef>, //current paramdef

    pub unnamed_rows: bool, // true = rows do not support names (Chromehounds)
    pub headerless_rows: bool, // Armored Core: FF

    pub rows: Vec<Row>, // load with `apply_paramdef` before using cells

    /// Private copy of the file to read row data from later.
    row_reader: Option<Vec<u8>>,
}

impl IO for Param {
    //Deserializes file data from a reader.
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let mut param = Param::default();

        reader.set_position(0x2C);

        let be = reader.matches(|r| r.read_u8(), &[0u8, 0xFFu8])?;

        param.big_endian = (be == 0xFF);
        reader.big_endian = param.big_endian;

        param.format_2d = FormatFlags1::from_bits_retain(reader.read_u8()?);
        param.format_2e = FormatFlags2::from_bits_retain(reader.read_u8()?);
        param.paramdef_format_version = reader.read_u8()?;

        reader.set_position(0);

        //make a private copy of the file to read row data from later
        let copy = reader.data.get_ref().to_vec();
        param.row_reader = Some(copy);

        let f = param.format_2d;
        let int_offset = f.contains(FormatFlags1::FLAG01) && f.contains(FormatFlags1::INT_DATA_OFFSET);
        let long_offset = f.contains(FormatFlags1::LONG_DATA_OFFSET);

        //the strings offset in the header is highly unreliable; only use it as a last resort
        let mut actual_strings_offset: i64 = 0;
        let strings_offset = reader.read_u32()? as i64;
        let mut data_start_header: i64 = -1;
        if int_offset || long_offset {
            reader.assert::<i16>(0)?;
        } else {
            data_start_header = reader.read_u16()? as i64; // data start
        }

        param.unk06 = reader.read_i16()?;
        param.paramdef_data_version = reader.read_i16()?;
        let row_count = reader.read_u16()? as usize;

        if f.contains(FormatFlags1::OFFSET_PARAM_TYPE) {
            reader.assert::<i32>(0)?;
            let param_type_offset = reader.read_i64()?;
            reader.assert_bytes(&[0u8; 0x14])?;

            //check if ParamTypeOffset is invalid and longer than file.
            if param_type_offset < reader.length() as i64 {
                reader.step_in(param_type_offset as u64)?;
                param.param_type = reader.read_ascii()?;
                reader.step_out()?;
                actual_strings_offset = param_type_offset;
            }
        } else {
            param.param_type = reader.read_shift_jis_fixed(0x20)?;
        }

        reader.skip(4); // Format
        if int_offset {
            data_start_header = reader.read_i32()? as i64; // Data start
            reader.assert::<i32>(0)?;
            reader.assert::<i32>(0)?;
            reader.assert::<i32>(0)?;
        } else if long_offset {
            data_start_header = reader.read_i64()?; // Data start
            reader.assert::<i64>(0)?;
        }

        //to detect nameless and headerless rows
        let rows_start = reader.position();
        let get_row_data_offset = |reader: &mut BinaryReader, position: u64|
         -> Result<i64, BinaryReaderError> {
            if long_offset {
                Ok(reader.get::<i64>(position + 8)? as i64)
            } else {
                Ok(reader.get::<i32>(position + 4)? as i64)
            }
        };

        //detect if row header has no name offset
        let row_data_offset1 = get_row_data_offset(reader, rows_start)?;
        let rows_size = row_data_offset1 - rows_start as i64;
        let mut row_header_size: i64 = 12;
        if rows_size < (row_count as i64 * row_header_size) {
            param.unnamed_rows = true;
            row_header_size = 8;
        }

        //detect if rows are headerless
        if data_start_header != -1
            && (rows_start == data_start_header as u64 || (rows_start + row_header_size as u64) > data_start_header as u64)
        {
            param.headerless_rows = true;
            param.unnamed_rows = true;
        }

        param.rows = Vec::with_capacity(row_count);
        if param.headerless_rows {
            if row_count == 0 {
                return Err(BinaryReaderError::invalid_data("headerless param with zero rows".to_string()));
            }

            param.detected_size = ((reader.length() - rows_start) / row_count as u64) as i64;
            let mut row_offset = rows_start;
            for i in 0..row_count {
                param.rows.push(Row::new_headerless(i as i32, row_offset as i64));
                row_offset += param.detected_size as u64;
            }
        } else {
            for _ in 0..row_count {
                let row = Row::from_reader(reader, &param, &mut actual_strings_offset)?; // must inspect param flags
                param.rows.push(row);
            }

            param.detected_size = match param.rows.len() {
                0 => -1,
                1 => {
                    let end = if actual_strings_offset == 0 { strings_offset } else { actual_strings_offset };
                    end - param.rows[0].data_offset
                }
                _ => param.rows[1].data_offset - param.rows[0].data_offset,
            };
        }

        Ok(param)
    }

    ///Serializes file data to a writer.
    fn into_writer(&mut self) -> Result<BinaryWriter, BinaryWriterError> {
        let mut writer = BinaryWriter::default();

        let applied = self.applied_paramdef.as_ref().ok_or_else(|| {
            BinaryWriterError::custom("Params cannot be written without applying a paramdef.")
        })?;

        writer.big_endian = self.big_endian;

        let f = self.format_2d;
        let int_offset = f.contains(FormatFlags1::FLAG01) && f.contains(FormatFlags1::INT_DATA_OFFSET);
        let long_offset = f.contains(FormatFlags1::LONG_DATA_OFFSET);

        writer.reserve::<u32>("StringsOffset");
        if int_offset || long_offset {
            writer.write_i16(0);
        } else {
            writer.reserve::<u16>("DataStart");
        }
        writer.write_i16(self.unk06);
        writer.write_i16(self.paramdef_data_version);

        if self.rows.len() > u16::MAX as usize {
            return Err(BinaryWriterError::Custom(format!(
                "Param \"{}\" has more than {} rows and cannot be saved.",
                applied.param_type,
                u16::MAX
            )));
        }
        writer.write_u16(self.rows.len() as u16);

        if f.contains(FormatFlags1::OFFSET_PARAM_TYPE) {
            writer.write_i32(0);
            writer.reserve::<i64>("ParamTypeOffset");
            writer.write_pattern(0x00, 0x14)?;
        } else {
            //padding isn't always accurate. Doesn't matter according to SFNext
            let padding: u8 = if f.contains(FormatFlags1::FLAG01) | self.headerless_rows {
                0x20 //ascii space
            } else {
                0x00 //null
            };

            writer.write_shift_jis_fixed(&self.param_type, 0x20, padding);
        }
        writer.write_u8(if self.big_endian { 0xFF } else { 0x00 });
        writer.write_u8(self.format_2d.bits());
        writer.write_u8(self.format_2e.bits());
        writer.write_u8(self.paramdef_format_version);
        if int_offset {
            writer.reserve::<u32>("DataStart");
            writer.write_i32(0);
            writer.write_i32(0);
            writer.write_i32(0);
        } else if long_offset {
            writer.reserve::<i64>("DataStart");
            writer.write_i64(0);
        }

        if !self.headerless_rows {
            for (i, row) in self.rows.iter().enumerate() {
                row.write_header(&mut writer, self, i)
                    .map_err(|e| BinaryWriterError::custom(e.to_string()));
            }
        }

        // "This is probably pretty stupid" - SFNext ?
        if self.format_2d == FormatFlags1::FLAG01 {
            writer.write_pattern(0x00, 0x20)?;
        }

        if int_offset {
            writer.fill::<u32>("DataStart", writer.position() as u32);
        } else if long_offset {
            writer.fill::<i64>("DataStart", writer.position() as i64);
        } else {
            writer.fill::<u16>("DataStart", writer.position() as u16);
        }

        for (i, row) in self.rows.iter().enumerate() {
            row.write_cells(&mut writer, self, i)
                .map_err(|e| BinaryWriterError::custom(e.to_string()));
        }

        writer.fill::<u32>("StringsOffset", writer.position() as u32);

        if f.contains(FormatFlags1::OFFSET_PARAM_TYPE) {
            writer.fill::<i64>("ParamTypeOffset", writer.position() as i64);
            writer.write_ascii(&self.param_type, true);
        }

        let mut string_offsets: HashMap<String, i64> = HashMap::new();
        string_offsets.insert(String::new(), writer.position() as i64);

        if !self.unnamed_rows && !self.headerless_rows {
            writer.write_i16(0); // null string
            for (i, row) in self.rows.iter().enumerate() {
                row.write_name(&mut writer, self, i, &mut string_offsets)
                    .map_err(|e| BinaryWriterError::custom(e.to_string()));
            }

            // DeS and BB sometimes (but not always) include some useless padding here - SFNext
            writer.write_i16(0); // useless padding at the end
        }

        Ok(writer)
    }

}

impl Param {
    ///Interprets row data according to the given paramdef and stores it for later writing.
    pub fn apply_paramdef(&mut self, paramdef: &Paramdef) -> Result<(), FerrisoulsError> {
        self.apply_paramdef_inner(paramdef, u64::MAX)
    }

    fn apply_paramdef_inner(&mut self, paramdef: &Paramdef, version: u64) -> Result<(), FerrisoulsError> {
        let data = self
            .row_reader
            .as_mut()
            .ok_or_else(|| FerrisoulsError::custom("Param has no row data to read"))?;
        let mut reader = BinaryReader::new(data, true, false);

        for row in &mut self.rows {
            row.read_cells(&mut reader, &Rc::new(paramdef.clone()), version)?;
        }
        self.applied_paramdef = Some(paramdef.clone());
        Ok(())
    }

    pub fn apply_regulation_versioned_paramdef(&mut self, paramdef: &Paramdef, version: u64) -> Result<(), FerrisoulsError> {
        if !paramdef.version_aware {
            return Err(FerrisoulsError::custom("PARAMDEF must be version aware to apply with a regulation version"));
        }
        self.apply_paramdef_inner(paramdef, version)
    }

    fn paramdef_score(&self,paramdef: &Paramdef, version: Option<u64>) -> u8 {
        let mut score = 0;

        if self.param_type == paramdef.param_type {
            score += 1;
        }

        if self.paramdef_data_version == paramdef.data_version {
            score += 1;
        }

        let size_matches = match version {
            Some(version) => {
                self.detected_size == -1
                    || self.detected_size == paramdef.get_row_size_versioned(version) as i64
            }
            None => {
                self.detected_size == -1
                    || self.detected_size == paramdef.get_row_size() as i64
            }
        };

        if size_matches {
            score += 1;
        }

        score
    }

    pub fn apply_best_paramdef<'b>(&mut self,
        paramdefs: impl IntoIterator<Item = &'b Paramdef>,
        min_score: u8,
        version: Option<u64>,
        require_version_aware: bool,
    ) -> Result<bool, FerrisoulsError> {

        let best = paramdefs
            .into_iter()
            .filter(|p| !require_version_aware || p.version_aware)
            .filter_map(|p| {
                let score = self.paramdef_score(p, version);

                (score >= min_score).then_some((score, p))
            })
            .max_by_key(|(score, _)| *score);

        if let Some((_, paramdef)) = best {
            match version {
                Some(version) => self.apply_regulation_versioned_paramdef(paramdef, version)?,
                None => self.apply_paramdef(paramdef)?,
            }

            return Ok(true);
        }

        Ok(false)
    }


    ///Returns the first row with the given ID, or None if not found.
    pub fn get(&self, id: i32) -> Option<&Row> {
        self.rows.iter().find(|row| row.id == id)
    }

    ///Mutable version of `get`.
    pub fn get_mut(&mut self, id: i32) -> Option<&mut Row> {
        self.rows.iter_mut().find(|row| row.id == id)
    }

}

impl BinderEntry for Param {
    type Identifier = Option<String>;

    fn identity(&self) -> &Self::Identifier {
        &self.name
    }
}

impl MetaEntry for Param {
    fn header(&self) -> Option<BND4EntryHeader> {
        self.header.clone()
    }

    fn name(&self) -> Option<String> {
        self.name.clone()
    }

    fn set_header(&mut self, header: &BND4EntryHeader) {
        self.header = Some(header.clone());
    }

    fn set_name(&mut self, name: &Option<String>) {
        self.name = name.clone();
    }
}

impl<'a> std::fmt::Display for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} v{} [{}]", self.param_type, self.paramdef_data_version, self.rows.len())
    }
}



pub struct ParamBND {
    header: BND4Header,
    entries: Vec<Param>
}


impl IO for ParamBND {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let mut bnd = BND4::from_reader(reader)?;
        Self::from_binder(&mut bnd)
            .map_err(|e| BinaryReaderError::Custom(e.to_string()))
    }

    fn into_writer(&mut self) -> Result<BinaryWriter, BinaryWriterError> {
        let mut writer = BinaryWriter::default();

        let mut bnd = unsafe { self.pack() }
            .map_err(|e| BinaryWriterError::Custom(e.to_string()))?;
        bnd.to_writer(&mut writer)?;

        Ok(writer)
    }

}

impl Binder for ParamBND {
    const VERSION: BinderVersion = BinderVersion::V4;
    type Entry = Param;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}

impl MetaBinder for ParamBND {
    fn header(&self) -> BND4Header {
        self.header.clone()
    }

    fn new(header: BND4Header, entries: Vec<<Self as Binder>::Entry>) -> Self {
        Self { header, entries }
    }
}

#[cfg(test)]
mod tests {
    use crate::{dcx::CompressSettings, oodle::{core::init_oodle, structs::OodleSettings}};
    use std::path::Path;
    use super::*;

    #[test]
    fn load_params() {
        unsafe {init_oodle(Path::new("tests/oo2core_6_win64.dll"));}

        let (mut fmg, dcx_type) = unsafe { ParamBND::unpack_binder(Path::new("tests/gameparam.parambnd.dcx")).unwrap() };

        for p in fmg.iter_mut() {
            println!("{}", p.param_type)
        }
    }
}
