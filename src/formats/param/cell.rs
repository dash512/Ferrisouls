use std::fmt;
use std::rc::Rc;

use crate::errors::FerrisoulsError;
use crate::formats::param::paramdef::{DefType, Field};

#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    S8(i8),
    U8(u8),
    S16(i16),
    U16(u16),
    S32(i32),
    U32(u32),
    F32(f32),
    F64(f64),
    Str(String),
    Bytes(Vec<u8>),
}

macro_rules! accessor {
    ($name:ident, $variant:ident, $ty:ty) => {
        pub fn $name(&self) -> Result<$ty, FerrisoulsError> {
            match self {
                CellValue::$variant(v) => Ok(v.clone()),
                other => Err(FerrisoulsError::Custom(format!(
                    "Cell value {other} is not of type {}",
                    stringify!($variant)
                ))),
            }
        }
    };
}

impl CellValue {
    accessor!(as_s8, S8, i8);
    accessor!(as_u8, U8, u8);
    accessor!(as_s16, S16, i16);
    accessor!(as_u16, U16, u16);
    accessor!(as_s32, S32, i32);
    accessor!(as_u32, U32, u32);
    accessor!(as_f32, F32, f32);
    accessor!(as_f64, F64, f64);
    accessor!(as_str, Str, String);
    accessor!(as_bytes, Bytes, Vec<u8>);

    pub fn to_integer(&self) -> Option<i128> {
        match self {
            CellValue::S8(v) => Some(*v as i128),
            CellValue::U8(v) => Some(*v as i128),
            CellValue::S16(v) => Some(*v as i128),
            CellValue::U16(v) => Some(*v as i128),
            CellValue::S32(v) => Some(*v as i128),
            CellValue::U32(v) => Some(*v as i128),
            CellValue::F32(v) if v.is_finite() => Some(v.round_ties_even() as i128),
            CellValue::F64(v) if v.is_finite() => Some(v.round_ties_even() as i128),
            CellValue::Str(s) => s.trim().parse::<i128>().ok(),
            _ => None,
        }
    }

    pub fn to_float(&self) -> Option<f64> {
        match self {
            CellValue::F32(v) => Some(*v as f64),
            CellValue::F64(v) => Some(*v),
            CellValue::Str(s) => s.trim().parse::<f64>().ok(),
            CellValue::Bytes(_) => None,
            other => other.to_integer().map(|i| i as f64),
        }
    }

    pub fn coerce(self, def: &Field) -> Result<CellValue, FerrisoulsError> {
        fn int<T: TryFrom<i128>>(v: &CellValue, ty: &str) -> Result<T, FerrisoulsError> {
            v.to_integer().and_then(|i| T::try_from(i).ok()).ok_or_else(|| {
                FerrisoulsError::Custom(format!("Value {v} cannot be converted to {ty}"))
            })
        }
        fn float(v: &CellValue, ty: &str) -> Result<f64, FerrisoulsError> {
            v.to_float()
                .ok_or_else(|| FerrisoulsError::Custom(format!("Value {v} cannot be converted to {ty}")))
        }
        fn bytes(v: CellValue) -> Result<CellValue, FerrisoulsError> {
            match v {
                CellValue::Bytes(_) => Ok(v),
                other => Err(FerrisoulsError::Custom(format!("Value {other} is not a byte array"))),
            }
        }

        Ok(match def.display_type {
            DefType::S8 => CellValue::S8(int(&self, "s8")?),
            DefType::U8 => {
                if def.array_length > 1 {
                    bytes(self)?
                } else {
                    CellValue::U8(int(&self, "u8")?)
                }
            }
            DefType::S16 => CellValue::S16(int(&self, "s16")?),
            DefType::U16 => CellValue::U16(int(&self, "u16")?),
            DefType::S32 => CellValue::S32(int(&self, "s32")?),
            DefType::U32 => CellValue::U32(int(&self, "u32")?),
            DefType::B32 => CellValue::S32(int(&self, "b32")?),
            DefType::F32 | DefType::Angle32 => CellValue::F32(float(&self, "f32")? as f32),
            DefType::F64 => CellValue::F64(float(&self, "f64")?),
            DefType::FixStr | DefType::FixStrW => CellValue::Str(self.to_string()),
            DefType::Dummy8 => {
                if def.bit_size == -1 {
                    bytes(self)?
                } else {
                    CellValue::U8(int(&self, "u8")?)
                }
            }
            #[allow(unreachable_patterns)]
            other => {
                return Err(FerrisoulsError::Custom(format!(
                    "Conversion not specified for type {other:?}"
                )))
            }
        })
    }
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellValue::S8(v) => write!(f, "{v}"),
            CellValue::U8(v) => write!(f, "{v}"),
            CellValue::S16(v) => write!(f, "{v}"),
            CellValue::U16(v) => write!(f, "{v}"),
            CellValue::S32(v) => write!(f, "{v}"),
            CellValue::U32(v) => write!(f, "{v}"),
            CellValue::F32(v) => write!(f, "{v}"),
            CellValue::F64(v) => write!(f, "{v}"),
            CellValue::Str(v) => write!(f, "{v}"),
            CellValue::Bytes(v) => {
                for b in v {
                    write!(f, "{b:02X}")?;
                }
                Ok(())
            }
        }
    }
}

/// One cell in one row in a param.
#[derive(Debug, Clone)]
pub struct Cell {
    pub def: Rc<Field>,
    value: CellValue,
}

impl Cell {
    pub fn new(def: Rc<Field>, value: CellValue) -> Result<Self, FerrisoulsError> {
        let value = value.coerce(&def)?;
        Ok(Cell { def, value })
    }

    /// The value of this cell.
    pub fn value(&self) -> &CellValue {
        &self.value
    }

    /// Sets the value, converting it to the type the field requires.
    pub fn set_value(&mut self, value: CellValue) -> Result<(), FerrisoulsError> {
        self.value = value.coerce(&self.def)?;
        Ok(())
    }

    pub fn display_type(&self) -> DefType {
        self.def.display_type
    }

    pub fn internal_name(&self) -> &str {
        &self.def.internal_name
    }

    pub fn array_length(&self) -> i32 {
        self.def.array_length
    }

    pub fn bit_size(&self) -> i32 {
        self.def.bit_size
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {} = {}", self.def.display_type, self.def.internal_name, self.value)
    }
}

