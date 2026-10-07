use std::collections::HashMap;
use std::rc::Rc;

use crate::binary::{BinaryReader, BinaryWriter};
use crate::errors::{BinaryReaderError, FerrisoulsError};
use crate::formats::param::param::{FormatFlags1, FormatFlags2, Param};
use crate::formats::param::cell::{Cell, CellValue};
use crate::formats::param::util;
use crate::formats::param::paramdef::{DefType, Paramdef};

/// One row in a param file.
#[derive(Debug)]
pub struct Row {
    pub id: i32,
    pub name: Option<String>,

    pub def: Option<Rc<Paramdef>>, //paramdef describing this row
    cells: Vec<Cell>, //must be loaded with `Param::apply_paramdef` first

    pub data_offset: i64,
}

impl Clone for Row {
    fn clone(&self) -> Self {
        Row {
            def: self.def.clone(),
            id: self.id,
            name: self.name.clone(),
            cells: self.cells.clone(),
            data_offset: 0,
        }
    }
}

impl Row {
    ///Creates a new row based on the given paramdef with default values.
    pub fn new(id: i32, name: Option<String>, paramdef: &Rc<Paramdef>) -> Result<Self, FerrisoulsError> {
        let mut cells = Vec::with_capacity(paramdef.fields.len());
        for field in &paramdef.fields {
            let value = util::convert_default_value(field)?;
            cells.push(Cell::new(field.clone(), value)?);
        }
        Ok(Row { def: Some(paramdef.clone()), id, name, cells, data_offset: 0 })
    }

    ///Creates a headerless row.
    pub fn new_headerless(id: i32, data_offset: i64) -> Self {
        Row { def: None, id, name: None, cells: Vec::new(), data_offset }
    }

    pub fn from_reader(reader: &mut BinaryReader, parent: &Param, actual_strings_offset: &mut i64) -> Result<Self, BinaryReaderError> {
        let id;
        let data_offset;
        let name_offset: i64;
        if parent.format_2d.contains(FormatFlags1::LONG_DATA_OFFSET) {
            id = reader.read_i32()?;
            reader.read_i32()?; // should be 0; not asserted cuz SOTFS can have garbage here. eg. generatordbglocation params
            data_offset = reader.read_i64()?;
            name_offset = if !parent.unnamed_rows { reader.read_i64()? } else { -1 };
        } else {
            id = reader.read_i32()?;
            data_offset = reader.read_u32()? as i64;
            name_offset = if !parent.unnamed_rows { reader.read_u32()? as i64 } else { -1 };
        }

        let mut name = None;
        if !parent.unnamed_rows && name_offset != 0 && name_offset != reader.length() as i64 {
            if *actual_strings_offset == 0 || name_offset < *actual_strings_offset {
                *actual_strings_offset = name_offset;
            }

            reader.step_in(name_offset as u64)?;
            name = Some(if parent.format_2e.contains(FormatFlags2::UNICODE_ROW_NAMES) {
                reader.read_utf16()?
            } else {
                reader.read_shift_jis()?
            });
            reader.step_out()?;
        }

        Ok(Row { def: None, id, name, cells: Vec::new(), data_offset })
    }

    pub fn read_cells(&mut self, reader: &mut BinaryReader, paramdef: &Rc<Paramdef>, regulation_version: u64) -> Result<(), FerrisoulsError> {
        // In case someone decides to add new rows before applying the paramdef (please don't do that)
        if self.data_offset == 0 {
            return Ok(());
        }

        self.def = Some(paramdef.clone());

        reader.set_position(self.data_offset as u64);
        let mut cells: Vec<Option<Cell>> = (0..paramdef.fields.len()).map(|_| None).collect();

        let mut bit_offset: i32 = -1;
        let mut bit_limit: i32 = -1;
        let mut bit_value: u64 = 0; // u64 so the orphaned-bits check doesn't fail on offsets of 32
        const BIT_VALUE_SIZE: i32 = 64;

        let id = self.id;
        let data_offset = self.data_offset;
        let check_orphaned_bits = |bit_offset: i32, bit_value: u64, pos: i64| -> Result<(), FerrisoulsError> {
            if bit_offset != -1 && (bit_value >> (bit_offset as u32)) != 0 {
                return Err(FerrisoulsError::Custom(format!(
                    "Invalid paramdef {}; bits would be lost before +0x{:X} in row {}.",
                    paramdef.param_type,
                    pos - data_offset,
                    id
                )));
            }
            Ok(())
        };

        for i in 0..paramdef.fields.len() {
            let field = &paramdef.fields[i];

            // For version aware PARAMDEFs, skip fields that don't exist in the specified version
            if paramdef.version_aware && !field.is_valid_for_regulation_version(regulation_version) {
                continue;
            }

            let mut value: Option<CellValue> = None;
            let ty = field.display_type;

            match ty {
                DefType::B32 => value = Some(CellValue::S32(reader.read_i32()?)),
                DefType::F32 | DefType::Angle32 => value = Some(CellValue::F32(reader.read_f32()?)),
                DefType::F64 => value = Some(CellValue::F64(reader.read_f64()?)),
                DefType::FixStr => {
                    value = Some(CellValue::Str(reader.read_shift_jis_fixed(field.array_length as usize)?))
                }
                DefType::FixStrW => {
                    value = Some(CellValue::Str(reader.read_utf16_fixed(field.array_length as usize * 2)?))
                }
                t if util::is_bit_type(t) => {
                    if field.bit_size == -1 {
                        value = Some(match t {
                            DefType::S8 => CellValue::S8(reader.read_i8()?),
                            DefType::U8 => {
                                if field.array_length > 1 {
                                    CellValue::Bytes(reader.read_bytes(field.array_length as usize)?)
                                } else {
                                    CellValue::U8(reader.read_u8()?)
                                }
                            }
                            DefType::S16 => CellValue::S16(reader.read_i16()?),
                            DefType::U16 => CellValue::U16(reader.read_u16()?),
                            DefType::S32 => CellValue::S32(reader.read_i32()?),
                            DefType::U32 => CellValue::U32(reader.read_u32()?),
                            DefType::Dummy8 => {
                                CellValue::Bytes(reader.read_bytes(field.array_length as usize)?)
                            }
                            _ => {
                                return Err(FerrisoulsError::Custom(format!(
                                    "Unexpected type in bitfield handling: {t:?}"
                                )))
                            }
                        });
                    }
                }
                t => return Err(FerrisoulsError::Custom(format!("Unsupported field type: {t:?}"))),
            }

            let value = match value {
                Some(v) => {
                    check_orphaned_bits(bit_offset, bit_value, reader.position() as i64)?;
                    bit_offset = -1;
                    v
                }
                None => {
                    if bit_offset == -1
                        || util::get_bit_limit(ty) != bit_limit
                        || bit_offset + field.bit_size > bit_limit
                    {
                        check_orphaned_bits(bit_offset, bit_value, reader.position() as i64)?;
                        bit_offset = 0;
                        bit_limit = util::get_bit_limit(ty);

                        // Always read unsigned to retain the exact bits; sign-extended later if applicable
                        bit_value = match bit_limit {
                            8 => reader.read_u8()? as u64,
                            16 => reader.read_u16()? as u64,
                            32 => reader.read_u32()? as u64,
                            _ => {
                                return Err(FerrisoulsError::Custom(format!(
                                    "Unexpected bit limit in bitfield handling: {bit_limit}"
                                )))
                            }
                        };
                    }

                    if field.bit_size == 0 {
                        return Err(FerrisoulsError::custom("Bit size 0 is not supported."));
                    }
                    if field.bit_size > bit_limit {
                        return Err(FerrisoulsError::Custom(format!(
                            "Bit size {} is too large to fit in type {ty:?}.",
                            field.bit_size
                        )));
                    }

                    let left_shift = (BIT_VALUE_SIZE - field.bit_size - bit_offset) as u32;
                    let right_shift = (BIT_VALUE_SIZE - field.bit_size) as u32;

                    let shifted: i64 = if util::is_signed_bit_type(ty) {
                        // Arithmetic shift on i64 for sign extension
                        ((bit_value as i64).wrapping_shl(left_shift)).wrapping_shr(right_shift)
                    } else {
                        // Logical shift on u64 to avoid sign extension
                        bit_value.wrapping_shl(left_shift).wrapping_shr(right_shift) as i64
                    };

                    bit_offset += field.bit_size;
                    match ty {
                        DefType::S8 => CellValue::S8(shifted as i8),
                        DefType::U8 => CellValue::U8(shifted as u8),
                        DefType::S16 => CellValue::S16(shifted as i16),
                        DefType::U16 => CellValue::U16(shifted as u16),
                        DefType::S32 => CellValue::S32(shifted as i32),
                        DefType::U32 => CellValue::U32(shifted as u32),
                        DefType::Dummy8 => CellValue::U8(shifted as u8),
                        _ => {
                            return Err(FerrisoulsError::Custom(format!(
                                "Unexpected type in bitfield handling: {ty:?}"
                            )))
                        }
                    }
                }
            };

            cells[i] = Some(Cell::new(field.clone(), value)?);
        }

        check_orphaned_bits(bit_offset, bit_value, reader.position() as i64)?;
        self.cells = cells.into_iter().flatten().collect();
        Ok(())
    }

    pub fn write_header(&self, writer: &mut BinaryWriter, parent: &Param, i: usize) -> Result<(), FerrisoulsError> {
        if parent.format_2d.contains(FormatFlags1::LONG_DATA_OFFSET) {
            writer.write_i32(self.id);
            writer.write_i32(0);
            writer.reserve::<i64>(&format!("RowOffset{i}"));

            // Likely won't be nameless at this point anyways
            if !parent.unnamed_rows {
                writer.reserve::<i64>(&format!("NameOffset{i}"));
            }
        } else {
            writer.write_i32(self.id);
            writer.reserve::<u32>(&format!("RowOffset{i}"));

            if !parent.unnamed_rows {
                writer.reserve::<u32>(&format!("NameOffset{i}"));
            }
        }
        Ok(())
    }

    pub fn write_cells(&self, writer: &mut BinaryWriter, parent: &Param, index: usize) -> Result<(), FerrisoulsError> {
        if !parent.headerless_rows {
            if parent.format_2d.contains(FormatFlags1::LONG_DATA_OFFSET) {
                writer.fill(&format!("RowOffset{index}"), writer.position());
            } else {
                writer.fill(&format!("RowOffset{index}"), writer.position() as u32);
            }
        }

        let mut bit_offset: i32 = -1;
        let mut bit_limit: i32 = -1;
        let mut bit_value: u64 = 0;
        const BIT_VALUE_SIZE: i32 = 64;

        for (i, cell) in self.cells.iter().enumerate() {
            let value = cell.value();
            let field = &cell.def;
            let ty = field.display_type;

            match ty {
                DefType::B32 => writer.write_i32(value.as_s32()?)?,
                DefType::F32 | DefType::Angle32 => writer.write_f32(value.as_f32()?)?,
                DefType::F64 => writer.write_f64(value.as_f64()?)?,
                DefType::FixStr => writer.write_shift_jis_fixed(&value.as_str()?, field.array_length as usize, 0)?,
                DefType::FixStrW => writer.write_utf16_fixed(&value.as_str()?, field.array_length as usize * 2, 0u8)?,
                t if util::is_bit_type(t) => {
                    if field.bit_size == -1 {
                        match t {
                            DefType::S8 => writer.write_i8(value.as_s8()?)?,
                            DefType::U8 => {
                                if field.array_length > 1 {
                                    writer.write_bytes(&value.as_bytes()?)?
                                } else {
                                    writer.write_u8(value.as_u8()?)?
                                }
                            }
                            DefType::S16 => writer.write_i16(value.as_s16()?)?,
                            DefType::U16 => writer.write_u16(value.as_u16()?)?,
                            DefType::S32 => writer.write_i32(value.as_s32()?)?,
                            DefType::U32 => writer.write_u32(value.as_u32()?)?,
                            DefType::Dummy8 => writer.write_bytes(&value.as_bytes()?)?,
                            _ => {
                                return Err(FerrisoulsError::Custom(format!(
                                    "Unexpected type in bitfield handling: {t:?}"
                                )))
                            }
                        }
                    } else {
                        if bit_offset == -1 {
                            bit_offset = 0;
                            bit_limit = util::get_bit_limit(t);
                            bit_value = 0;
                        }

                        let mut shifted: u64 = match t {
                            DefType::S8 => value.as_s8()? as u8 as u64,
                            DefType::U8 => value.as_u8()? as u64,
                            DefType::S16 => value.as_s16()? as u16 as u64,
                            DefType::U16 => value.as_u16()? as u64,
                            DefType::S32 => value.as_s32()? as u32 as u64,
                            DefType::U32 => value.as_u32()? as u64,
                            DefType::Dummy8 => value.as_u8()? as u64,
                            _ => {
                                return Err(FerrisoulsError::Custom(format!(
                                    "Unexpected type in bitfield handling: {t:?}"
                                )))
                            }
                        };

                        // Shift left first to clear any out-of-range bits
                        shifted = shifted
                            .wrapping_shl((BIT_VALUE_SIZE - field.bit_size) as u32)
                            .wrapping_shr((BIT_VALUE_SIZE - field.bit_size - bit_offset) as u32);
                        bit_value |= shifted;
                        bit_offset += field.bit_size;

                        let write = if i == self.cells.len() - 1 {
                            true
                        } else {
                            let next = &self.cells[i + 1].def;
                            let next_type = next.display_type;
                            !util::is_bit_type(next_type)
                                || next.bit_size == -1
                                || util::get_bit_limit(next_type) != bit_limit
                                || bit_offset + next.bit_size > bit_limit
                        };

                        if write {
                            bit_offset = -1;
                            match bit_limit {
                                8 => writer.write_u8(bit_value as u8)?,
                                16 => writer.write_u16(bit_value as u16)?,
                                32 => writer.write_u32(bit_value as u32)?,
                                _ => {
                                    return Err(FerrisoulsError::Custom(format!(
                                        "Unexpected bit limit in bitfield handling: {bit_limit}"
                                    )));
                                }
                            }
                        }
                    }
                }
                t => return Err(FerrisoulsError::Custom(format!("Unsupported field type: {t:?}"))),
            }
        }
        Ok(())
    }

    pub fn write_name(&self, writer: &mut BinaryWriter, parent: &Param, i: usize, string_offsets: &mut HashMap<String, i64>) -> Result<(), FerrisoulsError> {
        let name = self.name.as_deref().unwrap_or("");
        let mut name_offset = string_offsets.get(name).copied().unwrap_or(0);

        if name_offset == 0 {
            name_offset = writer.position() as i64;
            if parent.format_2e.contains(FormatFlags2::UNICODE_ROW_NAMES) {
                writer.write_utf16(name, true);
            } else {
                writer.write_shift_jis(name, true);
            }
            string_offsets.insert(name.to_string(), name_offset);
        }

        if parent.format_2d.contains(FormatFlags1::LONG_DATA_OFFSET) {
            writer.fill(&format!("NameOffset{i}"), name_offset);
        } else {
            writer.fill(&format!("NameOffset{i}"), name_offset as u32);
        }
        Ok(())
    }

    ///The cells of this row.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    ///Returns the first cell in the row with the given internal name.
    pub fn cell(&self, name: &str) -> Option<&Cell> {
        self.cells.iter().find(|c| c.def.internal_name == name)
    }

    pub fn cell_mut(&mut self, name: &str) -> Option<&mut Cell> {
        self.cells.iter_mut().find(|c| c.def.internal_name == name)
    }
}

impl std::fmt::Display for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.id, self.name.as_deref().unwrap_or(""))
    }
}

