//! Behavior of a UI element.

use crate::{binary::{BinaryReader, BinaryWriter}, errors::{BinaryReaderError, BinaryWriterError}};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ControlType {
    ///Unknown
    DmeCtrlScrollText,
    ///Unknown
    FrpgMenuDlgObjContentsHelpItem,
    ///Unknown
    #[default]
    Static,
    ///A simple button.
    Button,
    ///Unknown
    Cursor,
    ///Unknown
    LineHelpCtrl,
    ///Unknown
    ListCtrl,
    ///Unknown
    GridCtrl,
    ///Slider in options
    SliderCtrl,
    ///Unknown
    ChildDialogCtrl,
    ///Unknown
    ComboBoxCtrl,
}

impl ControlType {
    pub fn from_string(string: &str) -> Option<Self> {
        match string {
            "DmeCtrlScrollText" => Some(Self::DmeCtrlScrollText),
            "FrpgMenuDlgObjContentsHelpItem" => Some(Self::FrpgMenuDlgObjContentsHelpItem),
            "Static" => Some(Self::Static),
            "Button" => Some(Self::Button),
            "Cursor" => Some(Self::Cursor),
            "LineHelpCtrl" => Some(Self::LineHelpCtrl),
            "ListCtrl" => Some(Self::ListCtrl),
            "GridCtrl" => Some(Self::GridCtrl),
            "SliderCtrl" => Some(Self::SliderCtrl),
            "ChildDialogCtrl" => Some(Self::ChildDialogCtrl),
            "ComboBoxCtrl" => Some(Self::ComboBoxCtrl),
            _ => None,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Self::DmeCtrlScrollText => "DmeCtrlScrollText",
            Self::FrpgMenuDlgObjContentsHelpItem => "FrpgMenuDlgObjContentsHelpItem",
            Self::Static => "Static",
            Self::Button => "Button",
            Self::Cursor => "Cursor",
            Self::LineHelpCtrl => "LineHelpCtrl",
            Self::ListCtrl => "ListCtrl",
            Self::GridCtrl => "GridCtrl",
            Self::SliderCtrl => "SliderCtrl",
            Self::ChildDialogCtrl => "ChildDialogCtrl",
            Self::ComboBoxCtrl => "ComboBoxCtrl",
        }
    }
}


/// Determines the behavior of a UI element.
#[derive(Debug, Default)]
pub struct Control {
    ///Type-specific data. The control's type is derived from this.
    pub body: ControlBody,
    ///Offset of the type name in the STR block, kept so the string table can be written back verbatim.
    type_offset: u32,
}

impl Control {
    ///Wraps a body. `type_offset` must point at the matching type name in the file's STR block.
    pub fn new(body: ControlBody, type_offset: u32) -> Self {
        Self { body, type_offset }
    }

    pub fn control_type(&self) -> ControlType {
        self.body.control_type()
    }

    ///Reads the 8-byte CTRL header, then steps into `ctpr_start + ctpr_offset` for the body.
    pub fn from_reader(reader: &mut BinaryReader, ctpr_start: u64, str_start: u64) -> Result<Self, BinaryReaderError> {
        let type_offset = reader.read_u32()?;
        let ctpr_offset = reader.read_u32()?;
        let type_name = reader.get_utf16(str_start + type_offset as u64)?;

        let ctrl_type = ControlType::from_string(&type_name)
            .ok_or_else(|| format!("Unknown control type: {}", type_name))?;

        reader.step_in(ctpr_start + ctpr_offset as u64)?;
        let body = ControlBody::from_reader(reader, ctrl_type)?;
        reader.step_out()?;

        Ok(Self { body, type_offset })
    }

    ///Writes the CTRL header entry. `ctpr_offset` is where `write_data` put this body in CTPR.
    pub fn write_header(&self, writer: &mut BinaryWriter, ctpr_offset: u32) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.type_offset)?;
        writer.write_u32(ctpr_offset)
    }

    ///Writes the body into the CTPR blob.
    pub fn write_data(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        self.body.to_writer(writer)
    }
}


#[derive(Debug)]
pub enum ControlBody {
    ScrollTextDummy(ScrollTextDummy),
    HelpItem(HelpItem),
    Static(Static),
    Button(Button),
    Cursor(Cursor),
    LineHelpCtrl(LineHelpCtrl),
    ListCtrl(ListCtrl),
    GridCtrl(GridCtrl),
    SliderCtrl(SliderCtrl),
    ChildDialogCtrl(ChildDialogCtrl),
    ComboBoxCtrl(ComboBoxCtrl),
}

impl Default for ControlBody {
    fn default() -> Self {
        Self::Static(Static::default())
    }
}

impl ControlBody {
    pub fn control_type(&self) -> ControlType {
        match self {
            Self::ScrollTextDummy(_) => ControlType::DmeCtrlScrollText,
            Self::HelpItem(_) => ControlType::FrpgMenuDlgObjContentsHelpItem,
            Self::Static(_) => ControlType::Static,
            Self::Button(_) => ControlType::Button,
            Self::Cursor(_) => ControlType::Cursor,
            Self::LineHelpCtrl(_) => ControlType::LineHelpCtrl,
            Self::ListCtrl(_) => ControlType::ListCtrl,
            Self::GridCtrl(_) => ControlType::GridCtrl,
            Self::SliderCtrl(_) => ControlType::SliderCtrl,
            Self::ChildDialogCtrl(_) => ControlType::ChildDialogCtrl,
            Self::ComboBoxCtrl(_) => ControlType::ComboBoxCtrl,
        }
    }

    pub fn from_reader(reader: &mut BinaryReader, ctrl_type: ControlType) -> Result<Self, BinaryReaderError> {
        Ok(match ctrl_type {
            ControlType::DmeCtrlScrollText => Self::ScrollTextDummy(ScrollTextDummy::from_reader(reader)?),
            ControlType::FrpgMenuDlgObjContentsHelpItem => Self::HelpItem(HelpItem::from_reader(reader)?),
            ControlType::Static => Self::Static(Static::from_reader(reader)?),
            ControlType::Button => Self::Button(Button::from_reader(reader)?),
            ControlType::Cursor => Self::Cursor(Cursor::from_reader(reader)?),
            ControlType::LineHelpCtrl => Self::LineHelpCtrl(LineHelpCtrl::from_reader(reader)?),
            ControlType::ListCtrl => Self::ListCtrl(ListCtrl::from_reader(reader)?),
            ControlType::GridCtrl => Self::GridCtrl(GridCtrl::from_reader(reader)?),
            ControlType::SliderCtrl => Self::SliderCtrl(SliderCtrl::from_reader(reader)?),
            ControlType::ChildDialogCtrl => Self::ChildDialogCtrl(ChildDialogCtrl::from_reader(reader)?),
            ControlType::ComboBoxCtrl => Self::ComboBoxCtrl(ComboBoxCtrl::from_reader(reader)?),
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        match self {
            Self::ScrollTextDummy(c) => c.to_writer(writer),
            Self::HelpItem(c) => c.to_writer(writer),
            Self::Static(c) => c.to_writer(writer),
            Self::Button(c) => c.to_writer(writer),
            Self::Cursor(c) => c.to_writer(writer),
            Self::LineHelpCtrl(c) => c.to_writer(writer),
            Self::ListCtrl(c) => c.to_writer(writer),
            Self::GridCtrl(c) => c.to_writer(writer),
            Self::SliderCtrl(c) => c.to_writer(writer),
            Self::ChildDialogCtrl(c) => c.to_writer(writer),
            Self::ComboBoxCtrl(c) => c.to_writer(writer),
        }
    }
}

// ---------------------------------------------------------------------------
// Bodies made only of i32 fields
// ---------------------------------------------------------------------------

macro_rules! i32_control {
    ($(#[$meta:meta])* $name:ident { $($field:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, PartialEq, Eq)]
        pub struct $name {
            $(pub $field: i32,)+
        }

        impl $name {
            pub fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
                Ok(Self { $($field: reader.read_i32()?,)+ })
            }

            pub fn to_writer(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
                $(writer.write_i32(self.$field)?;)+
                Ok(())
            }
        }
    };
}

i32_control!(
    /// Unknown. `unk00` is always 0.
    ScrollTextDummy { unk00 }
);
i32_control!(
    /// Unknown. `unk00` is always 0.
    Static { unk00 }
);
i32_control!(
    /// Unknown.
    Cursor { unk00 }
);
i32_control!(
    /// Unknown. `unk00` is always 0.
    LineHelpCtrl { unk00 }
);
i32_control!(
    /// Unknown. `unk04` and `unk08` are always 8.
    ListCtrl { unk00, unk04, unk08 }
);
i32_control!(
    /// Unknown.
    GridCtrl { unk00, unk04, unk08, unk0c, unk10 }
);
i32_control!(
    /// Unknown. `unk00` is likely an ID.
    ChildDialogCtrl { unk00, unk04, unk08, unk0c, unk10, unk14 }
);


/// Unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpItem {
    pub unk00: i32,
    pub unk04: i32,
    pub unk08: i32,
    pub unk0c: i32,
    pub unk10: i32,
    pub unk14: i32,
    /// An FMG ID.
    pub text_id: i32,
}

impl Default for HelpItem {
    fn default() -> Self {
        Self { unk00: 0, unk04: 0, unk08: 0, unk0c: 0, unk10: 0, unk14: 0, text_id: -1 }
    }
}

impl HelpItem {
    pub fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self {
            unk00: reader.read_i32()?,
            unk04: reader.read_i32()?,
            unk08: reader.read_i32()?,
            unk0c: reader.read_i32()?,
            unk10: reader.read_i32()?,
            unk14: reader.read_i32()?,
            text_id: reader.read_i32()?,
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.unk00)?;
        writer.write_i32(self.unk04)?;
        writer.write_i32(self.unk08)?;
        writer.write_i32(self.unk0c)?;
        writer.write_i32(self.unk10)?;
        writer.write_i32(self.unk14)?;
        writer.write_i32(self.text_id)
    }
}

/// A button.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Button {
    pub unk00: i32,
    pub unk04: i16,
    pub unk06: u8,
    pub unk07: u8,
    pub unk08: i32,
    pub unk0c: i32,
    pub unk10: i32,
    pub unk14: i32,
    pub unk16: i16,
    pub unk17: u8,
    pub unk18: u8,
}

impl Button {
    pub fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self {
            unk00: reader.read_i32()?,
            unk04: reader.read_i16()?,
            unk06: reader.read_u8()?,
            unk07: reader.read_u8()?,
            unk08: reader.read_i32()?,
            unk0c: reader.read_i32()?,
            unk10: reader.read_i32()?,
            unk14: reader.read_i32()?,
            unk16: reader.read_i16()?,
            unk17: reader.read_u8()?,
            unk18: reader.read_u8()?,
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.unk00)?;
        writer.write_i16(self.unk04)?;
        writer.write_u8(self.unk06)?;
        writer.write_u8(self.unk07)?;
        writer.write_i32(self.unk08)?;
        writer.write_i32(self.unk0c)?;
        writer.write_i32(self.unk10)?;
        writer.write_i32(self.unk14)?;
        writer.write_i16(self.unk16)?;
        writer.write_u8(self.unk17)?;
        writer.write_u8(self.unk18)
    }
}

/// A slider in options.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SliderCtrl {
    pub unk00: i32,
    pub unk04: i16,
    pub unk06: i16,
    pub unk08: i32,
    pub unk0c: i32,
    pub unk10: i32,
    pub unk14: i32,
    /// Present in the C# class but never read or written by it, so it is not serialized here either.
    pub unk18: i32,
    pub unk22: i32,
    pub unk26: i32,
}

impl SliderCtrl {
    pub fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self {
            unk00: reader.read_i32()?,
            unk04: reader.read_i16()?,
            unk06: reader.read_i16()?,
            unk08: reader.read_i32()?,
            unk0c: reader.read_i32()?,
            unk10: reader.read_i32()?,
            unk14: reader.read_i32()?,
            unk18: 0,
            unk22: reader.read_i32()?,
            unk26: reader.read_i32()?,
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.unk00)?;
        writer.write_i16(self.unk04)?;
        writer.write_i16(self.unk06)?;
        writer.write_i32(self.unk08)?;
        writer.write_i32(self.unk0c)?;
        writer.write_i32(self.unk10)?;
        writer.write_i32(self.unk14)?;
        writer.write_i32(self.unk22)?;
        writer.write_i32(self.unk26)
    }
}

/// Unknown.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ComboBoxCtrl {
    pub unk00: i32,
    pub unk02: i16,
    pub unk04: i16,
    pub unk08: i32,
    pub unk0c: i32,
    pub unk10: i32,
    pub unk14: i32,
}

impl ComboBoxCtrl {
    pub fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self {
            unk00: reader.read_i32()?,
            unk02: reader.read_i16()?,
            unk04: reader.read_i16()?,
            unk08: reader.read_i32()?,
            unk0c: reader.read_i32()?,
            unk10: reader.read_i32()?,
            unk14: reader.read_i32()?,
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.unk00)?;
        writer.write_i16(self.unk02)?;
        writer.write_i16(self.unk04)?;
        writer.write_i32(self.unk08)?;
        writer.write_i32(self.unk0c)?;
        writer.write_i32(self.unk10)?;
        writer.write_i32(self.unk14)
    }
}

