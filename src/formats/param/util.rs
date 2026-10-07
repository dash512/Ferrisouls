use crate::errors::FerrisoulsError;
use crate::formats::param::cell::CellValue;
use crate::formats::param::paramdef::{DefType, EditFlags, Field, Paramdef};

pub fn get_default_format(ty: DefType) -> String {
    match ty {
        DefType::S8
        | DefType::U8
        | DefType::S16
        | DefType::U16
        | DefType::S32
        | DefType::U32
        | DefType::B32
        | DefType::FixStr
        | DefType::FixStrW => "%d",
        DefType::F32 | DefType::Angle32 | DefType::F64 => "%f",
        DefType::Dummy8 => "",
    }
    .to_string()
}

fn variable(def: Option<&Paramdef>) -> bool {
    def.map(|d| d.variable_editor_value_types()).unwrap_or(false)
}

fn f(v: f32) -> Option<CellValue> {
    Some(CellValue::F32(v))
}

fn i(v: i32) -> Option<CellValue> {
    Some(CellValue::S32(v))
}

pub fn get_default_default(def: Option<&Paramdef>, ty: DefType) -> Option<CellValue> {
    if variable(def) {
        match ty {
            DefType::F32 | DefType::Angle32 => f(0.0),
            DefType::F64 => Some(CellValue::F64(0.0)),
            DefType::Dummy8 | DefType::FixStr | DefType::FixStrW => None,
            _ => i(0),
        }
    } else {
        // Fixed editor values are always floats
        f(0.0)
    }
}

pub fn get_default_minimum(def: Option<&Paramdef>, ty: DefType) -> Option<CellValue> {
    if variable(def) {
        match ty {
            DefType::S8 => i(i8::MIN as i32),
            DefType::U8 => i(0),
            DefType::S16 => i(i16::MIN as i32),
            DefType::U16 => i(0),
            DefType::S32 => i(i32::MIN),
            DefType::U32 => i(0),
            DefType::B32 => i(0),
            DefType::F32 | DefType::Angle32 => f(f32::MIN),
            DefType::F64 => Some(CellValue::F64(f64::MIN)),
            DefType::Dummy8 | DefType::FixStr | DefType::FixStrW => None,
        }
    } else {
        f(match ty {
            DefType::S8 => i8::MIN as f32,
            DefType::U8 => 0.0,
            DefType::S16 => i16::MIN as f32,
            DefType::U16 => 0.0,
            DefType::S32 => -2147483520.0, // Smallest representable float greater than i32::MIN
            DefType::U32 => 0.0,
            DefType::B32 => 0.0,
            DefType::F32 | DefType::Angle32 | DefType::F64 => f32::MIN,
            DefType::Dummy8 => 0.0,
            DefType::FixStr | DefType::FixStrW => -1.0,
        })
    }
}

pub fn get_default_maximum(def: Option<&Paramdef>, ty: DefType) -> Option<CellValue> {
    if variable(def) {
        match ty {
            DefType::S8 => i(i8::MAX as i32),
            DefType::U8 => i(u8::MAX as i32),
            DefType::S16 => i(i16::MAX as i32),
            DefType::U16 => i(u16::MAX as i32),
            DefType::S32 => i(i32::MAX),
            DefType::U32 => i(i32::MAX), // Yes, u32 uses signed int too (usually)
            DefType::B32 => i(1),
            DefType::F32 | DefType::Angle32 => f(f32::MAX),
            DefType::F64 => Some(CellValue::F64(f64::MAX)),
            DefType::Dummy8 | DefType::FixStr | DefType::FixStrW => None,
        }
    } else {
        f(match ty {
            DefType::S8 => i8::MAX as f32,
            DefType::U8 => u8::MAX as f32,
            DefType::S16 => i16::MAX as f32,
            DefType::U16 => u16::MAX as f32,
            DefType::S32 => 2147483520.0, // Largest representable float less than i32::MAX
            DefType::U32 => 4294967040.0, // Largest representable float less than u32::MAX
            DefType::B32 => 1.0,
            DefType::F32 | DefType::Angle32 | DefType::F64 => f32::MAX,
            DefType::Dummy8 => 0.0,
            DefType::FixStr | DefType::FixStrW => 1_000_000_000.0,
        })
    }
}

pub fn get_default_increment(def: Option<&Paramdef>, ty: DefType) -> Option<CellValue> {
    if variable(def) {
        match ty {
            DefType::F32 | DefType::Angle32 => f(0.01),
            DefType::F64 => Some(CellValue::F64(0.01)),
            DefType::Dummy8 | DefType::FixStr | DefType::FixStrW => None,
            _ => i(1),
        }
    } else {
        f(match ty {
            DefType::F32 | DefType::Angle32 | DefType::F64 => 0.01,
            DefType::Dummy8 => 0.0,
            _ => 1.0,
        })
    }
}

pub fn get_default_edit_flags(ty: DefType) -> EditFlags {
    match ty {
        DefType::Dummy8 => EditFlags::empty(),
        _ => EditFlags::WRAP,
    }
}

pub fn is_array_type(ty: DefType) -> bool {
    matches!(
        ty,
        DefType::U8 // ACFA AcActRestrictionParam.def
            | DefType::Dummy8
            | DefType::FixStr
            | DefType::FixStrW
    )
}

pub fn is_bit_type(ty: DefType) -> bool {
    matches!(
        ty,
        DefType::S8 | DefType::U8 | DefType::S16 | DefType::U16 | DefType::S32 | DefType::U32 | DefType::Dummy8
    )
}

pub fn is_signed_bit_type(ty: DefType) -> bool {
    matches!(ty, DefType::S8 | DefType::S16 | DefType::S32)
}

pub fn get_value_size(ty: DefType) -> i32 {
    match ty {
        DefType::S8 | DefType::U8 | DefType::Dummy8 | DefType::FixStr => 1,
        DefType::S16 | DefType::U16 | DefType::FixStrW => 2,
        DefType::S32 | DefType::U32 | DefType::B32 | DefType::F32 | DefType::Angle32 => 4,
        DefType::F64 => 8,
    }
}

fn default_int<T: TryFrom<i128>>(v: &Option<CellValue>, ty: &str) -> Result<T, FerrisoulsError> {
    match v {
        None => T::try_from(0).map_err(|_| FerrisoulsError::Custom(format!("Cannot convert 0 to {ty}"))),
        Some(v) => v
            .to_integer()
            .and_then(|n| T::try_from(n).ok())
            .ok_or_else(|| FerrisoulsError::Custom(format!("Default {v} cannot be converted to {ty}"))),
    }
}

fn default_float(v: &Option<CellValue>, ty: &str) -> Result<f64, FerrisoulsError> {
    match v {
        None => Ok(0.0),
        Some(v) => v
            .to_float()
            .ok_or_else(|| FerrisoulsError::Custom(format!("Default {v} cannot be converted to {ty}"))),
    }
}

pub fn convert_default_value(field: &Field) -> Result<CellValue, FerrisoulsError> {
    let d = &field.default;
    Ok(match field.display_type {
        DefType::S8 => CellValue::S8(default_int(d, "s8")?),
        DefType::U8 => {
            let byte_default: u8 = default_int(d, "u8")?;
            if field.array_length > 1 {
                // Some dummy fields use this type
                CellValue::Bytes(vec![byte_default; field.array_length as usize])
            } else {
                CellValue::U8(byte_default)
            }
        }
        DefType::S16 => CellValue::S16(default_int(d, "s16")?),
        DefType::U16 => CellValue::U16(default_int(d, "u16")?),
        DefType::S32 | DefType::B32 => CellValue::S32(default_int(d, "s32")?),
        DefType::U32 => CellValue::U32(default_int(d, "u32")?),
        DefType::F32 | DefType::Angle32 => CellValue::F32(default_float(d, "f32")? as f32),
        DefType::F64 => CellValue::F64(default_float(d, "f64")?),
        DefType::FixStr | DefType::FixStrW => CellValue::Str(String::new()),
        DefType::Dummy8 => {
            if field.bit_size == -1 {
                CellValue::Bytes(vec![0; field.array_length as usize])
            } else {
                CellValue::U8(0)
            }
        }
    })
}

/// Panics if `ty` cannot be a bitfield. Callers always check `is_bit_type` first.
pub fn get_bit_limit(ty: DefType) -> i32 {
    match ty {
        DefType::S8 | DefType::U8 | DefType::Dummy8 => 8,
        DefType::S16 | DefType::U16 => 16,
        DefType::S32 | DefType::U32 => 32,
        _ => panic!("Type {ty} cannot be a bitfield."),
    }
}

