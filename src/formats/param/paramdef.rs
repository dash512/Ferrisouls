use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::LazyLock;

use bitflags::bitflags;
use regex::Regex;

use crate::binary::{BinaryReader, BinaryWriter};
use crate::errors::FerrisoulsError;
use crate::formats::param::cell::CellValue;
use crate::formats::param::util;



/// Supported primitive field types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefType {
    S8,
    U8,
    S16,
    U16,
    S32,
    U32,
    /// 4-byte integer representing a boolean.
    B32,
    /// Single-precision floating point value.
    F32,
    /// Single-precision floating point value representing an angle.
    Angle32,
    /// Double-precision floating point value.
    F64,
    /// Byte or array of bytes used for padding or placeholding.
    Dummy8,
    /// Fixed-width Shift-JIS string.
    FixStr,
    /// Fixed-width UTF-16 string.
    FixStrW,
}

impl DefType {
    /// The name used in the binary and XML formats (C# `ToString()`).
    pub fn as_str(&self) -> &'static str {
        match self {
            DefType::S8 => "s8",
            DefType::U8 => "u8",
            DefType::S16 => "s16",
            DefType::U16 => "u16",
            DefType::S32 => "s32",
            DefType::U32 => "u32",
            DefType::B32 => "b32",
            DefType::F32 => "f32",
            DefType::Angle32 => "angle32",
            DefType::F64 => "f64",
            DefType::Dummy8 => "dummy8",
            DefType::FixStr => "fixstr",
            DefType::FixStrW => "fixstrW",
        }
    }
}

impl FromStr for DefType {
    type Err = FerrisoulsError;
    fn from_str(s: &str) -> Result<Self, FerrisoulsError> {
        Ok(match s {
            "s8" => DefType::S8,
            "u8" => DefType::U8,
            "s16" => DefType::S16,
            "u16" => DefType::U16,
            "s32" => DefType::S32,
            "u32" => DefType::U32,
            "b32" => DefType::B32,
            "f32" => DefType::F32,
            "angle32" => DefType::Angle32,
            "f64" => DefType::F64,
            "dummy8" => DefType::Dummy8,
            "fixstr" => DefType::FixStr,
            "fixstrW" => DefType::FixStrW,
            _ => return Err(FerrisoulsError::Custom(format!("Unknown paramdef type: {s:?}"))),
        })
    }
}

impl std::fmt::Display for DefType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

bitflags! {
    /// Flags that control editor behavior for a field.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct EditFlags: i32 {
        /// Value wraps around when scrolled past the minimum or maximum.
        const WRAP = 1;
        /// Value may not be edited.
        const LOCK = 4;
    }
}




///A companion format to params that describes each field present in the rows.
///Extension: .def, .paramdef
#[derive(Debug, Clone)]
pub struct Paramdef {
    pub data_version: i16, // indicates row data structure
    pub param_type: String,
    pub big_endian: bool, // only true for PS3/Xbox360
    pub unicode: bool, // true = utf-16 strings, false = shift-jis
    /// Determines format of the file.
    /// 
    /// 0 - Armored Core Formula Front PS2, possibly not used yet  
    /// 
    /// 101 - Enchanted Arms, Chromehounds, AC4/FA/V/VD, Shadow Assault: Tenchu
    ///   
    /// 102 - Demon's Souls
    /// 
    /// 103 - Ninja Blade, Another Century's Episode: R
    /// 
    /// 104 - Dark Souls, Steel Battalion: Heavy Armor
    /// 
    /// 106 - Elden Ring (deprecated ObjectParam)
    /// 
    /// 201 - Bloodborne
    /// 
    /// 202 - Dark Souls 3
    /// 
    /// 203 - Elden Ring, Armored Core 6
    pub format_version: i16,
    /// Fields in each param row, in order of appearance.
    ///
    /// Fields are shared with cells via `Rc`; to edit one in place use `Rc::make_mut`.
    pub fields: Vec<Rc<Field>>,
    /// PARAMDEF is "regulation version aware" and can be applied to older regulation params
    /// that may have a different layout than the latest params if the XML paramdef supports it.
    pub version_aware: bool,
    /// Only basic fields are present. Used in Armored Core Formula Front for PS2.
    pub basic_fields: bool,
}

///PARAMDEF formatted for DS1.
impl Default for Paramdef {
    fn default() -> Self {
        Paramdef {
            data_version: 0,
            param_type: String::new(),
            big_endian: false,
            unicode: false,
            format_version: 104,
            fields: Vec::new(),
            version_aware: false,
            basic_fields: false,
        }
    }
}

///True for the formats that store strings via offsets (versions 106..200 and 202+).
fn offset_strings(format_version: i16) -> bool {
    format_version >= 202 || (106..200).contains(&format_version)
}

impl Paramdef {
    pub fn new() -> Self {
        Self::default()
    }

    ///Whether field default, minimum, maximum, and increment may be variable type.
    ///If false, they are always floats.
    pub fn variable_editor_value_types(&self) -> bool {
        self.format_version >= 203
    }

    ///Deserializes file data from a reader.
    pub fn from_reader(reader: &mut BinaryReader) -> Result<Self, FerrisoulsError> {
        let mut paramdef = Paramdef::default();

        paramdef.big_endian = reader.read_value_at::<i8>(0x2C)? == -1;
        reader.big_endian = paramdef.big_endian;
        paramdef.format_version = reader.read_value_at::<i16>(0x2E)?;
        reader.varint_long = paramdef.format_version >= 200;
        let fv = paramdef.format_version;

        reader.read_i32()?; // File size
        let header_size = reader.matches(|r| r.read_i16(), &[0x30, 0xFF])?;
        paramdef.data_version = reader.read_i16()?;
        let field_count = reader.read_i16()?;
        let field_size = reader.matches(
            |r| r.read_i16(), 
            &[0x48, 0x68, 0x6C, 0x88, 0x8C, 0xAC, 0xB0, 0xD0]
        )?;

        if fv >= 202 {
            reader.assert::<i32>(0)?;

            //NOTE: is there a reason for shift-his instead of ASCII here?
            let offset = reader.read_i64()?;
            paramdef.param_type = reader.get_shift_jis(offset as u64)?;
            reader.assert::<i64>(0)?;
            reader.assert::<i64>(0)?;
            reader.assert::<i32>(0)?;
        } else if (106..200).contains(&fv) {
            let offset = reader.read_i32()? as i64;
            paramdef.param_type = reader.get_shift_jis(offset as u64)?;
            reader.assert::<i64>(0)?;
            reader.assert::<i64>(0)?;
            reader.assert::<i64>(0)?;
            reader.assert::<i32>(0)?;
        } else {
            paramdef.param_type = reader.read_shift_jis_fixed(0x20)?;
        }

        reader.matches(|r| r.read_i8(), &[0, -1])?; // big endian
        paramdef.unicode = reader.read_boolean()?;
        reader.matches(|r| r.read_i16(), &[0, 101, 102, 103, 104, 106, 201, 202, 203])?; // format version
        if fv >= 200 {
            reader.assert::<i64>(0x38)?;
        }

        if !((fv < 200 && header_size == 0x30) || (fv >= 200 && header_size == 0xFF)) {
            return Err(FerrisoulsError::Custom(format!(
                "Unexpected header size 0x{header_size:X} for version {fv}."
            )));
        }

        paramdef.basic_fields = fv == 0 && field_size == 0x68;

        // Please note that for version 103 this value is wrong.
        let field_size_ok = paramdef.basic_fields
            || matches!(
                (fv, field_size),
                (101, 0x8C)
                    | (102, 0xAC)
                    | (103, 0x6C)
                    | (104, 0xB0)
                    | (106, 0x48)
                    | (201, 0xD0)
                    | (202, 0x68)
                    | (203, 0x88)
            );
        if !field_size_ok {
            return Err(FerrisoulsError::Custom(format!(
                "Unexpected field size 0x{field_size:X} for version {fv}."
            )));
        }

        paramdef.fields = Vec::with_capacity(field_count.max(0) as usize);
        for _ in 0..field_count {
            let field = Field::from_reader(reader, &paramdef)?;
            paramdef.fields.push(Rc::new(field));
        }
        Ok(paramdef)
    }

    /// Verifies that the file can be written safely.
    pub fn validate(&self) -> Result<(), FerrisoulsError> {
        let fv = self.format_version;
        let ok = (self.basic_fields && fv == 0) || matches!(fv, 101 | 102 | 103 | 104 | 106 | 201 | 202 | 203);
        if !ok {
            return Err(FerrisoulsError::Custom(format!("Unsupported version: {fv}")));
        }
        Ok(())
    }



    /// Serializes file data to a writer.
    pub fn write(&self, writer: &mut BinaryWriter) -> Result<(), FerrisoulsError> {
        if self.version_aware {
            return Err(FerrisoulsError::custom("Version aware PARAMDEFs cannot be written as binary."));
        }

        let fv = self.format_version;
        writer.big_endian = self.big_endian;
        writer.varint_long = fv >= 200;

        writer.reserve::<i32>("FileSize");
        writer.write_i16(if fv >= 200 { 0xFF } else { 0x30 });
        writer.write_i16(self.data_version);
        writer.write_i16(self.fields.len() as i16);

        let field_size: i16 = if self.basic_fields && fv == 0 {
            0x68
        } else {
            match fv {
                101 => 0x8C,
                102 => 0xAC,
                103 => 0x6C,
                104 => 0xB0,
                106 => 0x48,
                201 => 0xD0,
                202 => 0x68,
                203 => 0x88,
                _ => {
                    return Err(FerrisoulsError::Custom(format!(
                        "Unsupported format version: {fv}"
                    )))
                }
            }
        };
        writer.write_i16(field_size);

        if fv >= 202 {
            writer.write_i32(0);
            writer.reserve_varint("ParamTypeOffset");
            writer.write_i64(0);
            writer.write_i64(0);
            writer.write_i32(0);
        } else if (106..200).contains(&fv) {
            writer.reserve_varint("ParamTypeOffset");
            writer.write_i64(0);
            writer.write_i64(0);
            writer.write_i64(0);
            writer.write_i32(0);
        } else {
            writer.write_shift_jis_fixed(&self.param_type, 0x20, if fv >= 200 { 0x00 } else { 0x20 });
        }

        writer.write_i8(if self.big_endian { -1 } else { 0 });
        writer.write_boolean(self.unicode);
        writer.write_i16(fv);
        if fv >= 200 {
            writer.write_i64(0x38);
        }

        for (i, field) in self.fields.iter().enumerate() {
            field.to_writer(writer, self, i)?;
        }

        if offset_strings(fv) {
            writer.fill_varint("ParamTypeOffset", writer.position() as i64);
            writer.write_shift_jis(&self.param_type, true);
        }

        let field_strings_start = writer.position();
        let mut shared_string_offsets: HashMap<String, i64> = HashMap::new();
        for (i, field) in self.fields.iter().enumerate() {
            field.write_strings(writer, self, i, &mut shared_string_offsets);
        }

        // This entire heuristic seems extremely dubious
        if fv == 104 || fv == 201 {
            let len = writer.position() - field_strings_start;
            if len % 0x10 != 0 {
                writer.write_pattern((0x10 - len % 0x10) as u8, 0x00);
            }
        } else {
            if fv >= 202 && writer.position() % 0x10 == 0 {
                writer.write_pattern(0x10, 0x00);
            }
            writer.pad(0x10);
        }
        writer.fill("FileSize", writer.position() as i32);
        Ok(())
    }

    /// Calculates the size of cell data for each row.
    pub fn get_row_size(&self) -> i32 {
        self.get_row_size_versioned(u64::MAX)
    }

    /// Calculates the size of cell data for each row for a given regulation version.
    pub fn get_row_size_versioned(&self, version: u64) -> i32 {
        self.get_fields_size(self.fields.len(), version)
            .expect("field count equals fields.len()")
    }

    pub fn get_fields_size(&self, field_count: usize, version: u64) -> Result<i32, FerrisoulsError> {
        if field_count > self.fields.len() {
            return Err(FerrisoulsError::custom(
                "Count must be from 0 to total fields count.",
            ));
        }

        let mut size: i32 = 0;
        let mut i = 0usize;
        while i < field_count {
            if self.version_aware && !self.fields[i].is_valid_for_regulation_version(version) {
                i += 1;
                continue;
            }
            let field = &self.fields[i];
            let ty = field.display_type;
            if util::is_array_type(ty) {
                size += util::get_value_size(ty) * field.array_length;
            } else {
                size += util::get_value_size(ty);
            }

            if util::is_bit_type(ty) && field.bit_size != -1 {
                let mut bit_offset = field.bit_size;
                let bit_limit = util::get_bit_limit(ty);

                // Advances the outer index past fields packed into the same bitfield
                while i + 1 < field_count {
                    let next = &self.fields[i + 1];
                    let next_type = next.display_type;
                    if !util::is_bit_type(next_type)
                        || next.bit_size == -1
                        || util::get_bit_limit(next_type) != bit_limit
                        || bit_offset + next.bit_size > bit_limit
                    {
                        break;
                    }
                    bit_offset += next.bit_size;
                    i += 1;
                }
            }
            i += 1;
        }
        Ok(size)
    }

    /// If this PARAMDEF is version aware, returns a filtered PARAMDEF with only the fields that
    /// are valid for a specific regulation version. Underlying fields are shared, not cloned.
    pub fn get_filtered_paramdef_for_regulation_version(&self, version: u64) -> Result<Paramdef, FerrisoulsError> {
        if !self.version_aware {
            return Err(FerrisoulsError::custom("Version aware PARAMDEF required for filtering"));
        }
        Ok(Paramdef {
            data_version: self.data_version,
            param_type: self.param_type.clone(),
            big_endian: self.big_endian,
            unicode: self.unicode,
            format_version: self.format_version,
            fields: self
                .fields
                .iter()
                .filter(|f| f.is_valid_for_regulation_version(version))
                .cloned()
                .collect(),
            version_aware: false,
            basic_fields: false,
        })
    }
}

impl std::fmt::Display for Paramdef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} v{}", self.param_type, self.data_version)
    }
}




static ARRAY_LENGTH_RX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?paramdef<name>.+?)\s*\[\s*(?paramdef<length>\d+)\s*\]\s*$").unwrap());
static BIT_SIZE_RX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?paramdef<name>.+?)\s*:\s*(?paramdef<size>\d+)\s*$").unwrap());


///Information about a field present in each row in a param.
#[derive(Debug, Clone)]
pub struct Field {
    pub display_name: String,
    pub display_type: DefType,
    pub display_format: String, // format string

    pub default: Option<CellValue>, // default value for new rows
    pub minimum: Option<CellValue>,
    pub maximum: Option<CellValue>,
    pub increment: Option<CellValue>,

    pub edit_flags: EditFlags, // field behaviour flags
    /// Number of elements for array types; only supported for dummy8, fixstr, and fixstrW.
    pub array_length: i32,

    pub description: Option<String>,
    /// Type of the value in the engine; may be an enum type.
    pub internal_type: String,
    /// Name of the value in the engine; not present before version 102.
    pub internal_name: String,
    /// Number of bits used by a bitfield; only supported for unsigned types, -1 when not used.
    pub bit_size: i32,
    pub sort_id: i32, // order fields by this value. Not present before v104

    pub unk_b8: Option<String>, // identifier? Only supported in versions >= 200 (seen in 202).
    pub unk_c0: Option<String>, // param type? Only supported in versions >= 200 (seen in 202).
    pub unk_c8: Option<String>, // display string? Only supported in versions >= 200 (seen in 202).

    /// The first regulation version this field was introduced in, or 0 if it has always existed.
    /// Only exists in XML paramdefs.
    pub first_regulation_version: u64,
    /// The first regulation version this field is removed from, or 0 if never removed.
    /// Only exists in XML paramdefs.
    pub removed_regulation_version: u64,
}

///Creates a Field with placeholder values.
impl Default for Field {
    fn default() -> Self {
        Field::new(None, DefType::F32, "placeholder")
    }
}

// Helpers mirroring `Convert.ToXxx(object)` where null converts to zero.
fn opt_f32(v: &Option<CellValue>) -> Result<f32, FerrisoulsError> {
    match v {
        None => Ok(0.0),
        Some(v) => v
            .to_float()
            .map(|f| f as f32)
            .ok_or_else(|| FerrisoulsError::Custom(format!("Cannot convert {v} to f32"))),
    }
}
fn opt_f64(v: &Option<CellValue>) -> Result<f64, FerrisoulsError> {
    match v {
        None => Ok(0.0),
        Some(v) => v
            .to_float()
            .ok_or_else(|| FerrisoulsError::Custom(format!("Cannot convert {v} to f64"))),
    }
}
fn opt_i32(v: &Option<CellValue>) -> Result<i32, FerrisoulsError> {
    match v {
        None => Ok(0),
        Some(v) => v
            .to_integer()
            .and_then(|i| i32::try_from(i).ok())
            .ok_or_else(|| FerrisoulsError::Custom(format!("Cannot convert {v} to i32"))),
    }
}

impl Field {
    pub fn new(def: Option<&Paramdef>, display_type: DefType, internal_name: &str) -> Self {
        Field {
            display_name: internal_name.to_string(),
            display_type,
            display_format: util::get_default_format(display_type),
            default: util::get_default_default(def, display_type),
            minimum: util::get_default_minimum(def, display_type),
            maximum: util::get_default_maximum(def, display_type),
            increment: util::get_default_increment(def, display_type),
            edit_flags: util::get_default_edit_flags(display_type),
            array_length: 1,
            description: None,
            internal_type: display_type.to_string(),
            internal_name: internal_name.to_string(),
            bit_size: -1,
            sort_id: 0,
            unk_b8: None,
            unk_c0: None,
            unk_c8: None,
            first_regulation_version: 0,
            removed_regulation_version: 0,
        }
    }

    pub fn from_reader(reader: &mut BinaryReader, def: &Paramdef) -> Result<Self, FerrisoulsError> {
        let fv = def.format_version;

        let display_name = if offset_strings(fv) {
            let offset = reader.read_varint()?;
            reader.get_utf16(offset as u64)?
        } else if def.unicode {
            reader.read_utf16_fixed(0x40)?
        } else {
            reader.read_shift_jis_fixed(0x40)?
        };

        let display_type = DefType::from_str(&reader.read_shift_jis_fixed(8)?)?;
        let display_format = reader.read_shift_jis_fixed(8)?;

        let (mut default, mut minimum, mut maximum, mut increment)
            = (None, None, None, None);

        if fv >= 203 {
            reader.assert_pattern(0x00, 0x10)?;
        } else {
            default = Some(CellValue::F32(reader.read_f32()?));
            minimum = Some(CellValue::F32(reader.read_f32()?));
            maximum = Some(CellValue::F32(reader.read_f32()?));
            increment = Some(CellValue::F32(reader.read_f32()?));
        }

        let edit_flags = EditFlags::from_bits_retain(reader.read_i32()?);

        let byte_count = reader.read_i32()?;
        let value_size = util::get_value_size(display_type);
        let is_array = util::is_array_type(display_type);
        if (!is_array && byte_count != value_size) || (is_array && byte_count % value_size != 0) {
            return Err(FerrisoulsError::Custom(format!(
                "Unexpected byte count {byte_count} for type {display_type}."
            )));
        }
        let mut array_length = byte_count / value_size;

        let mut field = Field {
            display_name,
            display_type,
            display_format,
            default,
            minimum,
            maximum,
            increment,
            edit_flags,
            array_length,
            description: None,
            internal_type: String::new(),
            internal_name: String::new(),
            bit_size: -1,
            sort_id: 0,
            unk_b8: None,
            unk_c0: None,
            unk_c8: None,
            first_regulation_version: 0,
            removed_regulation_version: 0,
        };

        if def.basic_fields {
            return Ok(field);
        }

        let description_offset = reader.read_varint()?;
        if description_offset != 0 {
            field.description = Some(if def.unicode {
                reader.get_utf16(description_offset as u64)?
            } else {
                reader.get_shift_jis(description_offset as u64)?
            });
        }

        field.internal_type = if offset_strings(fv) {
            let offset = reader.read_varint()?;
            reader.get_ascii(offset as u64)?.trim().to_string()
        } else {
            reader.read_shift_jis_fixed(0x20)?.trim().to_string()
        };

        field.bit_size = -1;
        if fv >= 102 {
            let mut internal_name = if offset_strings(fv) {
                let offset = reader.read_varint()?;
                reader.get_ascii(offset as u64)?.trim().to_string()
            } else {
                reader.read_shift_jis_fixed(0x20)?.trim().to_string()
            };

            let bit_match = BIT_SIZE_RX.captures(&internal_name).map(|c| {
                (c["name"].to_string(), c["size"].parse::<i32>())
            });
            if let Some((name, size)) = bit_match {
                internal_name = name;
                field.bit_size = size.map_err(|e| FerrisoulsError::Custom(e.to_string()))?;
            }

            if is_array {
                let caps = ARRAY_LENGTH_RX.captures(&internal_name).map(|c| {
                    (c["name"].to_string(), c["length"].parse::<i32>())
                });
                let length = match &caps {
                    Some((_, l)) => *l.as_ref().map_err(|e| FerrisoulsError::Custom(e.to_string()))?,
                    None => 1,
                };
                if length != array_length {
                    // AcActRestrictionParam.def in Armored Core V has its last field with an internal
                    // name of reserved[8] and type of u8, but its byte count is 1, and the data only has
                    // 1 byte as well. So this exception is skipped for u8/dummy8. - SFNext
                    if display_type != DefType::U8 && display_type != DefType::Dummy8 {
                        return Err(FerrisoulsError::Custom(format!(
                            "Mismatched array length in {internal_name} with byte count {byte_count}."
                        )));
                    }

                    // Should probably trust the byte count over the name, but just in case...
                    array_length = array_length.min(length);
                    field.array_length = array_length;
                }

                if let Some((name, _)) = caps {
                    internal_name = name;
                }
            }
            field.internal_name = internal_name;
        }

        if fv >= 104 {
            field.sort_id = reader.read_i32()?;
        }

        if fv >= 200 {
            reader.assert::<i32>(0)?;
            let unk_b8 = reader.read_i64()?;
            let unk_c0 = reader.read_i64()?;
            let unk_c8 = reader.read_i64()?;

            if unk_b8 != 0 {
                field.unk_b8 = Some(reader.get_ascii(unk_b8 as u64)?);
            }
            if unk_c0 != 0 {
                field.unk_c0 = Some(reader.get_ascii(unk_c0 as u64)?);
            }
            if unk_c8 != 0 {
                field.unk_c8 = Some(reader.get_utf16(unk_c8 as u64)?);
            }
        } else if fv >= 106 {
            reader.assert::<i32>(0)?;
            reader.assert::<i32>(0)?;
            reader.assert::<i32>(0)?;
        }

        if fv >= 203 {
            fn read_variable_value(reader: &mut BinaryReader, ty: DefType) -> Result<Option<CellValue>, FerrisoulsError> {
                Ok(match ty {
                    DefType::S8
                    | DefType::U8
                    | DefType::S16
                    | DefType::U16
                    | DefType::S32
                    | DefType::U32
                    | DefType::B32 => {
                        let v = reader.read_i32()?;
                        reader.assert::<i32>(0)?;
                        Some(CellValue::S32(v))
                    }
                    DefType::F32 | DefType::Angle32 => {
                        let v = reader.read_f32()?;
                        reader.assert::<i32>(0)?;
                        Some(CellValue::F32(v))
                    }
                    DefType::F64 => Some(CellValue::F64(reader.read_f64()?)),
                    // Given that there are 8 bytes available, these could possibly be offsets
                    DefType::Dummy8 | DefType::FixStr | DefType::FixStrW => {
                        reader.assert::<i64>(0)?;
                        None
                    }
                })
            }

            field.default = read_variable_value(reader, display_type)?;
            field.minimum = read_variable_value(reader, display_type)?;
            field.maximum = read_variable_value(reader, display_type)?;
            field.increment = read_variable_value(reader, display_type)?;
        }

        Ok(field)
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, def: &Paramdef, index: usize) -> Result<(), FerrisoulsError> {
        let fv = def.format_version;

        if offset_strings(fv) {
            writer.reserve_varint(&format!("DisplayNameOffset{index}"));
        } else if def.unicode {
            writer.write_utf16_fixed(&self.display_name, 0x40, if fv >= 104 { 0x00 } else { 0x20 });
        } else {
            writer.write_shift_jis_fixed(&self.display_name, 0x40, if fv >= 104 { 0x00 } else { 0x20 });
        }

        let padding: u8 = if fv >= 106 { 0x00 } else { 0x20 };
        writer.write_shift_jis_fixed(self.display_type.as_str(), 8, padding);
        writer.write_shift_jis_fixed(&self.display_format, 8, padding);

        if fv >= 203 {
            writer.write_pattern(0x10, 0x00);
        } else {
            writer.write_f32(opt_f32(&self.default)?);
            writer.write_f32(opt_f32(&self.minimum)?);
            writer.write_f32(opt_f32(&self.maximum)?);
            writer.write_f32(opt_f32(&self.increment)?);
        }

        writer.write_i32(self.edit_flags.bits());
        let count = if util::is_array_type(self.display_type) { self.array_length } else { 1 };
        writer.write_i32(util::get_value_size(self.display_type) * count);
        if def.basic_fields {
            return Ok(());
        }

        writer.reserve_varint(&format!("DescriptionOffset{index}"));

        if offset_strings(fv) {
            writer.reserve_varint(&format!("InternalTypeOffset{index}"));
        } else {
            writer.write_shift_jis_fixed(&self.internal_type, 0x20, padding);
        }

        if offset_strings(fv) {
            writer.reserve_varint(&format!("InternalNameOffset{index}"));
        } else if fv >= 102 {
            writer.write_shift_jis_fixed(&self.make_internal_name(), 0x20, padding);
        }

        if fv >= 104 {
            writer.write_i32(self.sort_id);
        }

        if fv >= 200 {
            writer.write_i32(0);
            writer.reserve::<i64>(&format!("UnkB8Offset{index}"));
            writer.reserve::<i64>(&format!("UnkC0Offset{index}"));
            writer.reserve::<i64>(&format!("UnkC8Offset{index}"));
        } else if fv >= 106 {
            writer.write_i32(0);
            writer.write_i32(0);
            writer.write_i32(0);
        }

        if fv >= 203 {
            let write_variable_value = |writer: &mut BinaryWriter, value: &Option<CellValue>|
            -> Result<(), FerrisoulsError> {
                match self.display_type {
                    DefType::S8
                    | DefType::U8
                    | DefType::S16
                    | DefType::U16
                    | DefType::S32
                    | DefType::U32
                    | DefType::B32 => {
                        writer.write_i32(opt_i32(value)?)?;
                        writer.write_i32(0)?;
                    }
                    DefType::F32 | DefType::Angle32 => {
                        writer.write_f32(opt_f32(value)?)?;
                        writer.write_i32(0)?;
                    }
                    DefType::F64 => writer.write_f64(opt_f64(value)?)?,
                    DefType::Dummy8 | DefType::FixStr | DefType::FixStrW => writer.write_i64(0)?,
                }
                Ok(())
            };

            write_variable_value(writer, &self.default)?;
            write_variable_value(writer, &self.minimum)?;
            write_variable_value(writer, &self.maximum)?;
            write_variable_value(writer, &self.increment)?;
        }
        Ok(())
    }

    pub fn write_strings(&self, writer: &mut BinaryWriter, def: &Paramdef, index: usize, shared_string_offsets: &mut HashMap<String, i64>) {
        let fv = def.format_version;

        if offset_strings(fv) {
            writer.fill_varint(&format!("DisplayNameOffset{index}"), writer.position() as i64);
            writer.write_utf16(&self.display_name, true);
        }

        if def.basic_fields {
            return;
        }

        let mut description_offset = 0;
        if let Some(desc) = &self.description {
            description_offset = writer.position();
            if def.unicode {
                writer.write_utf16(desc, true);
            } else {
                writer.write_shift_jis(desc, true);
            }
        }
        writer.fill_varint(&format!("DescriptionOffset{index}"), description_offset as i64);

        if offset_strings(fv) {
            writer.fill_varint(&format!("InternalTypeOffset{index}"), writer.position() as i64);
            writer.write_ascii(&self.internal_type, true);

            writer.fill_varint(&format!("InternalNameOffset{index}"), writer.position() as i64);
            writer.write_ascii(&self.make_internal_name(), true);
        }

        if fv >= 200 {
            fn write_shared_string_maybe(writer: &mut BinaryWriter, offsets: &mut HashMap<String, i64>, s: Option<&str>, unicode: bool) -> i64 {
                let Some(s) = s else { return 0 };
                if let Some(&offset) = offsets.get(s) {
                    return offset;
                }
                let offset = writer.position() as i64;
                offsets.insert(s.to_string(), offset);
                if unicode {
                    writer.write_utf16(s, true);
                } else {
                    writer.write_ascii(s, true);
                }
                offset
            }

            let o = write_shared_string_maybe(writer, shared_string_offsets, self.unk_b8.as_deref(), false);
            writer.fill(&format!("UnkB8Offset{index}"), o);
            let o = write_shared_string_maybe(writer, shared_string_offsets, self.unk_c0.as_deref(), false);
            writer.fill(&format!("UnkC0Offset{index}"), o);
            let o = write_shared_string_maybe(writer, shared_string_offsets, self.unk_c8.as_deref(), true);
            writer.fill(&format!("UnkC8Offset{index}"), o);
        }
    }

    ///If the paramdef is version aware, returns whether this field is valid for a given regulation version.
    pub fn is_valid_for_regulation_version(&self, version: u64) -> bool {
        version >= self.first_regulation_version
            && (self.removed_regulation_version == 0 || version < self.removed_regulation_version)
    }

    fn make_internal_name(&self) -> String {
        // This formatting is almost 100% accurate in DS1, less so in BB, and a complete crapshoot in DS3
        if self.bit_size != -1 {
            format!("{}:{}", self.internal_name, self.bit_size)
        } else if util::is_array_type(self.display_type) {
            format!("{}[{}]", self.internal_name, self.array_length)
        } else {
            self.internal_name.clone()
        }
    }

    pub fn fits_game_version(&self, version: u64) -> bool {
        version == 0
            || (self.first_regulation_version <= version
                && (self.removed_regulation_version == 0 || self.removed_regulation_version > version))
    }
}

impl std::fmt::Display for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if util::is_bit_type(self.display_type) && self.bit_size != -1 {
            write!(f, "{} {}:{}", self.display_type, self.internal_name, self.bit_size)
        } else if util::is_array_type(self.display_type) {
            write!(f, "{} {}[{}]", self.display_type, self.internal_name, self.array_length)
        } else {
            write!(f, "{} {}", self.display_type, self.internal_name)
        }
    }
}

