use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::errors::{BinaryReaderError, BinaryWriterError};
use crate::formats::drb::DRBVersion;
use bitflags::bitflags;


#[derive(Debug, Default, PartialEq, Eq)]
pub enum ShapeType {
    Dialog,
    GouraudFrame,
    GouraudRect,
    GouraudSprite,
    Mask,
    MonoFrame,
    MonoRect,
    #[default]
    Null,
    ScrollText,
    Sprite,
    Text,
    ScriptCtrlDialog
}

impl ShapeType {
    pub fn from_string(string: &str) -> Option<Self> {
        match string {
            "Dialog" => Some(Self::Dialog),
            "GouraudFrame" => Some(Self::GouraudFrame),
            "GouraudRect" => Some(Self::GouraudRect),
            "GouraudSprite" => Some(Self::GouraudSprite),
            "Mask" => Some(Self::Mask),
            "MonoFrame" => Some(Self::MonoFrame),
            "MonoRect" => Some(Self::MonoRect),
            "Null" => Some(Self::Null),
            "ScrollText" => Some(Self::ScrollText),
            "Sprite" => Some(Self::Sprite),
            "Text" => Some(Self::Text),
            "ScriptCtrlDialog" => Some(Self::ScriptCtrlDialog),
            _ => None,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Self::Dialog => "Dialog",
            Self::GouraudFrame => "GouraudFrame",
            Self::GouraudSprite => "GouraudSprite",
            Self::GouraudRect => "GouraudRect",
            Self::Mask => "Mask",
            Self::MonoFrame => "MonoFrame",
            Self::MonoRect => "MonoRect",
            Self::Null => "Null",
            Self::ScrollText => "ScrollText",
            Self::Sprite => "Sprite",
            Self::Text => "Text",
            Self::ScriptCtrlDialog => "ScriptCtrlDialog"
        }
    }
}

#[repr(u8)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    ///No transparency
    Opaque,
    ///Alpha transparency
    #[default]
    Alpha,
    ///Color is added to the underlying color value
    Add,
    ///Color is subtracted from the underlying color value
    Subtract
}

impl BlendMode {
    pub fn from_u8(int: u8) -> Result<Self, &'static str> {
        match int {
            0 => Ok(Self::Opaque),
            1 => Ok(Self::Alpha),
            2 => Ok(Self::Add),
            3 => Ok(Self::Subtract),
            _=> Err("Invalid u8 for BlendMode")
        }
    }

    pub fn to_u8(&self) -> u8 { *self as u8 }
}

//Sprite Orientation / Align Flags
bitflags! {
    /// Rotation and mirroring flags of a Sprite.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct SpriteOrientation: u8 {
        /// Rotate texture 90 degrees clockwise
        const ROTATE_CW = 0x10;
        /// Rotate texture 180 degrees
        const ROTATE_180 = 0x20;
        /// Flip texture vertically
        const FLIP_VERTICAL = 0x40;
        /// Flip texture horizontally
        const FLIP_HORIZONTAL = 0x80;
    }

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct AlignFlags: u8 {
        const TOPLEFT = 0; // empty set
        const RIGHT = 1;
        const CENTER_HORIZONTAL = 2;
        const BOTTOM = 4;
        const CENTER_VERTICAL = 8;
    }
}

///ARGB color as stored in DRB files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color { a: 255, r: 0, g: 0, b: 0 };
    pub const WHITE: Color = Color { a: 255, r: 255, g: 255, b: 255 };
}

impl IO for Color {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let v = reader.read_u32()?;
        Ok(Self { a: v as u8, r: (v >> 24) as u8, g: (v >> 16) as u8, b: (v >> 8) as u8 })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        let v = self.a as u32 | (self.r as u32) << 24 | (self.g as u32) << 16 | (self.b as u32) << 8;
        writer.write_u32(v)
    }
}

impl Default for Color {
    fn default() -> Self { Self::BLACK }
}


///Indicates the source of a text element's text.
#[repr(u8)]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum TxtType {
    /// Text is a literal value stored in the DRB
    Literal = 0,
    /// Text is a static FMG ID
    #[default]
    FMG = 1,
    /// Text is assigned at runtime
    Dynamic = 2,
}

impl TxtType {
    pub fn from_u8(int: u8) -> Result<Self, &'static str> {
        match int {
            0 => Ok(Self::Literal),
            1 => Ok(Self::FMG),
            2 => Ok(Self::Dynamic),
            _ => Err("Invalid u8 for TxtType"),
        }
    }

    pub fn to_u8(&self) -> u8 { *self as u8 }
}


///Describes the appearance of a UI element.
#[derive(Debug, Default)]
pub struct Shape {
    shape_type: ShapeType,
    type_offset: u32,

    shpr_offset: u32,

    left_edge: u16,
    top_edge: u16,
    right_edge: u16,
    bottom_edge: u16,

    origin_x: i16,
    origin_y: i16,

    scaling_mode: u16,

    /// Type-specific data.
    pub body: ShapeBody,
}

impl Shape {
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion, shpr_start: u64, str_start: u64) -> Result<Self, BinaryReaderError> {
        let type_offset = reader.read_u32()?;
        let shpr_offset = reader.read_u32()?;

        let shape_type = reader.get_utf16(str_start + type_offset as u64)?;

        let mut shape = match ShapeType::from_string(&shape_type) {
            None => Err(format!("Invalid Shape Type: {:?} (type_offset={:#X})", shape_type, type_offset)),
            Some(shape_type) => Ok(Self {
                shape_type,
                type_offset,
                shpr_offset,
                ..Default::default()
            })
        }?;

        reader.step_in(shpr_start + shpr_offset as u64)?;

        shape.left_edge = reader.read_u16()?;
        shape.top_edge = reader.read_u16()?;
        shape.right_edge = reader.read_u16()?;
        shape.bottom_edge = reader.read_u16()?;

        if version == DRBVersion::DSR && shape.shape_type != ShapeType::Null {
            shape.origin_x = reader.read_i16()?;
            shape.origin_y = reader.read_i16()?;
            shape.scaling_mode = reader.read_u16()?;
            reader.read::<u16>()?;
        } else {
            shape.origin_x = -1;
            shape.origin_y = -1;
            shape.scaling_mode = 0;
        }

        shape.body = ShapeBody::from_reader(reader, version, &shape.shape_type)?;

        reader.step_out()?;

        Ok(shape)
    }
    
    pub fn write_header(&mut self, writer: &mut BinaryWriter, shpr_offset: u32) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.type_offset)?;
        writer.write_u32(shpr_offset)?;

        Ok(())
    }

    pub fn write_data(&mut self, writer: &mut BinaryWriter, version: DRBVersion) -> Result<(), BinaryWriterError> {
        writer.write_u16(self.left_edge)?;
        writer.write_u16(self.top_edge)?;
        writer.write_u16(self.right_edge)?;
        writer.write_u16(self.bottom_edge)?;

        if version == DRBVersion::DSR && self.shape_type != ShapeType::Null {
            writer.write_i16(self.origin_x)?;
            writer.write_i16(self.origin_y)?;
            writer.write_u16(self.scaling_mode)?;
            writer.write_u16(0)?;
        }

        self.body.to_writer(writer)
    }
}


#[derive(Debug, Default)]
pub enum ShapeBody {
    Dialog(Dialog),
    GouraudFrame(GouraudFrame),
    GouraudRect(GouraudRect),
    GouraudSprite(GouraudSprite),
    Mask(Mask),
    MonoFrame(MonoFrame),
    MonoRect(MonoRect),
    #[default]
    Null,
    ScrollText(TextBase),
    Sprite(Sprite),
    Text(TextBase),
    ScriptCtrlDialog(ScriptCtrlDialog),
}

impl ShapeBody {
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion, shape_type: &ShapeType) -> Result<Self, BinaryReaderError> {
        Ok(match shape_type {
            ShapeType::Dialog => Self::Dialog(Dialog::from_reader(reader, version)?),
            ShapeType::GouraudFrame => Self::GouraudFrame(GouraudFrame::from_reader(reader, version)?),
            ShapeType::GouraudRect => Self::GouraudRect(GouraudRect::from_reader(reader, version)?),
            ShapeType::GouraudSprite => Self::GouraudSprite(GouraudSprite::from_reader(reader)?),
            ShapeType::Mask => Self::Mask(Mask::from_reader(reader)?),
            ShapeType::MonoFrame => Self::MonoFrame(MonoFrame::from_reader(reader, version)?),
            ShapeType::MonoRect => Self::MonoRect(MonoRect::from_reader(reader, version)?),
            ShapeType::Null => Self::Null,
            ShapeType::ScrollText => Self::ScrollText(TextBase::from_reader(reader, version, true)?),
            ShapeType::Sprite => Self::Sprite(Sprite::from_reader(reader)?),
            ShapeType::Text => Self::Text(TextBase::from_reader(reader, version, false)?),
            ShapeType::ScriptCtrlDialog => Self::ScriptCtrlDialog(ScriptCtrlDialog::from_reader(reader)?),
        })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        match self {
            Self::Dialog(s) => s.to_writer(writer),
            Self::GouraudFrame(s) => s.to_writer(writer),
            Self::GouraudRect(s) => s.to_writer(writer),
            Self::GouraudSprite(s) => s.to_writer(writer),
            Self::Mask(s) => s.to_writer(writer),
            Self::MonoFrame(s) => s.to_writer(writer),
            Self::MonoRect(s) => s.to_writer(writer),
            Self::Null => Ok(()),
            Self::ScrollText(s) => s.to_writer(writer),
            Self::Sprite(s) => s.to_writer(writer),
            Self::Text(s) => s.to_writer(writer),
            Self::ScriptCtrlDialog(s) => s.to_writer(writer),
        }
    }
}


///Displays the region of a Texture.
#[derive(Debug, Default)]
pub struct SpriteBase {
    index: u16,

    left_edge: u16,
    top_edge: u16,
    right_edge: u16,
    bottom_edge: u16,

    blend_mode: BlendMode,
    orientation: SpriteOrientation,
}

impl IO for SpriteBase {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let left_edge = reader.read_u16()?;
        let top_edge = reader.read_u16()?;
        let right_edge = reader.read_u16()?;
        let bottom_edge = reader.read_u16()?;

        let index = reader.read_u16()?;

        let orientation = SpriteOrientation::from_bits_retain(reader.read_u8()?);
        let blend_mode = BlendMode::from_u8(reader.read_u8()?)?;

        Ok(Self { left_edge, top_edge, right_edge, bottom_edge, index, orientation, blend_mode })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u16(self.left_edge)?;
        writer.write_u16(self.top_edge)?;
        writer.write_u16(self.right_edge)?;
        writer.write_u16(self.bottom_edge)?;

        writer.write_u16(self.index)?;

        writer.write_u8(self.orientation.bits())?;
        writer.write_u8(self.blend_mode.to_u8())?;

        Ok(())
    }
}


///Displays a texture region with color tinting.
#[derive(Debug)]
pub struct Sprite {
    pub base: SpriteBase,
    /// Index of a color in the menu color param, or 0 to use the custom color.
    pub palette_color: i32,
    /// Custom color used when palette_color is 0.
    pub custom_color: Color,
}

impl Default for Sprite {
    fn default() -> Self {
        Self { base: SpriteBase::default(), palette_color: 0, custom_color: Color::WHITE }
    }
}

impl IO for Sprite {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let base = SpriteBase::from_reader(reader)?;
        let palette_color = reader.read_i32()?;
        let custom_color = Color::from_reader(reader)?;
        Ok(Self { base, palette_color, custom_color })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        self.base.to_writer(writer)?;
        writer.write_i32(self.palette_color)?;
        self.custom_color.to_writer(writer)
    }
}


///Displays a texture region with color interpolated between each corner.
#[derive(Debug)]
pub struct GouraudSprite {
    pub base: SpriteBase,
    pub top_left_color: Color,
    pub top_right_color: Color,
    pub bottom_right_color: Color,
    pub bottom_left_color: Color,
}

impl Default for GouraudSprite {
    fn default() -> Self {
        Self {
            base: SpriteBase::default(),
            top_left_color: Color::WHITE,
            top_right_color: Color::WHITE,
            bottom_right_color: Color::WHITE,
            bottom_left_color: Color::WHITE,
        }
    }
}

impl IO for GouraudSprite {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let base = SpriteBase::from_reader(reader)?;
        Ok(Self {
            base,
            top_left_color: Color::from_reader(reader)?,
            top_right_color: Color::from_reader(reader)?,
            bottom_right_color: Color::from_reader(reader)?,
            bottom_left_color: Color::from_reader(reader)?,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        self.base.to_writer(writer)?;
        self.top_left_color.to_writer(writer)?;
        self.top_right_color.to_writer(writer)?;
        self.bottom_right_color.to_writer(writer)?;
        self.bottom_left_color.to_writer(writer)
    }
}

// ---------------------------------------------------------------------------
// Dialog
// ---------------------------------------------------------------------------

///Unknown optional structure of Dialog.
#[derive(Debug, Default)]
pub struct DialogUnknownA {
    pub unk00: i16,
    pub unk02: i16,
    pub unk04: i32,
}

impl IO for DialogUnknownA {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self {
            unk00: reader.read_i16()?,
            unk02: reader.read_i16()?,
            unk04: reader.read_i32()?,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i16(self.unk00)?;
        writer.write_i16(self.unk02)?;
        writer.write_i32(self.unk04)
    }
}


///Displays another referenced group of elements.
#[derive(Debug)]
pub struct Dialog {
    /// Index of the Dlg referenced by this element, in the parent DRB's Dlg list.
    pub dlg_index: i16,
    /// Unknown; always 0 or 1.
    pub unk02: u8,
    /// Unknown; always 0 or 1.
    pub unk03: u8,
    /// Index of a color in the menu color param, or 0 to use the custom color.
    pub palette_color: i32,
    /// Custom color used when palette_color is 0.
    pub custom_color: Color,
    /// Unknown. SFNext asserts 0, but values of 1 have been seen before.
    pub unk10: u8,
    /// Unknown, optional, unused structure.
    pub unknown: Option<DialogUnknownA>,
}

impl Default for Dialog {
    fn default() -> Self {
        Self {
            dlg_index: -1,
            unk02: 1,
            unk03: 1,
            palette_color: 0,
            custom_color: Color::WHITE,
            unk10: 0,
            unknown: None,
        }
    }
}

impl Dialog {
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion) -> Result<Self, BinaryReaderError> {
        let dlg_index = reader.read_i16()?;
        let unk02 = reader.read_u8()?;
        let unk03 = reader.read_u8()?;
        let palette_color = reader.read_i32()?;
        let custom_color = Color::from_reader(reader)?;

        let mut unk0c = false;
        let mut unk10 = 0; // defaulted, could be 1.
        
        if version == DRBVersion::ACFA {
            // TODO: ACFA: What was this?
            reader.read_u8()?;
            reader.read_u8()?;
            reader.read_u8()?;
            reader.read_u8()?;
        } else {
            unk0c = reader.read_u8()? != 0;
            reader.assert::<u8>(0)?;
            reader.assert::<u8>(0)?;
            unk10 = reader.read::<u8>()?; // asserted 0 in SFNext, seems to be incorrect?
        }

        let unknown = if unk0c { Some(DialogUnknownA::from_reader(reader)?) } else { None };

        Ok(Self { dlg_index, unk02, unk03, palette_color, custom_color, unk10, unknown })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i16(self.dlg_index)?;
        writer.write_u8(self.unk02)?;
        writer.write_u8(self.unk03)?;
        writer.write_i32(self.palette_color)?;
        self.custom_color.to_writer(writer)?;

        // TODO: ACFA
        writer.write_u8(self.unknown.is_some() as u8)?;
        writer.write_u8(0)?;
        writer.write_u8(0)?;
        writer.write_u8(self.unk10)?;

        if let Some(mut unknown) = self.unknown.as_mut() {
            unknown.to_writer(writer)?;
        }
        Ok(())
    }
}


///A rectangular frame with color interpolated between each corner.
#[derive(Debug)]
pub struct GouraudFrame {
    pub blend_mode: BlendMode,
    pub thickness: u8,

    pub top_left_color: Color,
    pub top_right_color: Color,
    pub bottom_right_color: Color,
    pub bottom_left_color: Color,
}

impl Default for GouraudFrame {
    fn default() -> Self {
        Self {
            blend_mode: BlendMode::Alpha,
            thickness: 1,
            top_left_color: Color::BLACK,
            top_right_color: Color::BLACK,
            bottom_right_color: Color::BLACK,
            bottom_left_color: Color::BLACK,
        }
    }
}

impl GouraudFrame {
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion) -> Result<Self, BinaryReaderError> {
        let blend_mode = BlendMode::from_u8(reader.read_u8()?)?;
        if version == DRBVersion::ACFA {
            reader.read_u16()?; // TODO: ACFA: What was this?
        } else {
            reader.assert::<u16>(0)?;
        }
        let thickness = reader.read_u8()?;
        Ok(Self {
            blend_mode,
            thickness,
            top_left_color: Color::from_reader(reader)?,
            top_right_color: Color::from_reader(reader)?,
            bottom_right_color: Color::from_reader(reader)?,
            bottom_left_color: Color::from_reader(reader)?,
        })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u8(self.blend_mode.to_u8())?;
        writer.write_u16(0)?; // TODO: ACFA
        writer.write_u8(self.thickness)?;
        self.top_left_color.to_writer(writer)?;
        self.top_right_color.to_writer(writer)?;
        self.bottom_right_color.to_writer(writer)?;
        self.bottom_left_color.to_writer(writer)
    }
}

///A rectangle with color interpolated between each corner.
#[derive(Debug)]
pub struct GouraudRect {
    pub blend_mode: BlendMode,

    pub top_left_color: Color,
    pub top_right_color: Color,
    pub bottom_right_color: Color,
    pub bottom_left_color: Color,
}

impl Default for GouraudRect {
    fn default() -> Self {
        Self {
            blend_mode: BlendMode::Alpha,
            top_left_color: Color::BLACK,
            top_right_color: Color::BLACK,
            bottom_right_color: Color::BLACK,
            bottom_left_color: Color::BLACK,
        }
    }
}

impl GouraudRect {
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion) -> Result<Self, BinaryReaderError> {
        let blend_mode = BlendMode::from_u8(reader.read_u8()?)?;
        if version == DRBVersion::ACFA {
            // TODO: ACFA: What was this?
            reader.read_u16()?;
            reader.read_u8()?;
        } else {
            reader.assert::<u16>(0)?;
            reader.assert::<u8>(0)?;
        }
        Ok(Self {
            blend_mode,
            top_left_color: Color::from_reader(reader)?,
            top_right_color: Color::from_reader(reader)?,
            bottom_right_color: Color::from_reader(reader)?,
            bottom_left_color: Color::from_reader(reader)?,
        })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u8(self.blend_mode.to_u8())?;
        // TODO: ACFA
        writer.write_u16(0)?;
        writer.write_u8(0)?;
        self.top_left_color.to_writer(writer)?;
        self.top_right_color.to_writer(writer)?;
        self.bottom_right_color.to_writer(writer)?;
        self.bottom_left_color.to_writer(writer)
    }
}


///Unknown.
#[derive(Debug)]
pub struct Mask {
    /// Unknown. Offset into the string table is kept so it can be written back.
    pub dlgo_name: String,
    dlgo_name_offset: u32,
    /// Unknown; always 1.
    pub unk04: u8,
    /// Unknown; always 0.
    pub unk05: i32,
    /// Unknown; always 0.
    pub unk09: i32,
    /// Unknown; always 0.
    pub unk0d: i16,
}

impl Default for Mask {
    fn default() -> Self {
        Self { dlgo_name: String::new(), dlgo_name_offset: 0, unk04: 1, unk05: 0, unk09: 0, unk0d: 0 }
    }
}

impl IO for Mask {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let dlgo_name_offset = reader.read_u32()?;
        let dlgo_name = reader.get_utf16(dlgo_name_offset as u64)?;
        Ok(Self {
            dlgo_name,
            dlgo_name_offset,
            unk04: reader.read_u8()?,
            unk05: reader.read_i32()?,
            unk09: reader.read_i32()?,
            unk0d: reader.read_i16()?,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.dlgo_name_offset)?;
        writer.write_u8(self.unk04)?;
        writer.write_i32(self.unk05)?;
        writer.write_i32(self.unk09)?;
        writer.write_i16(self.unk0d)
    }
}


///A rectangular frame with a solid color.
#[derive(Debug)]
pub struct MonoFrame {
    pub blend_mode: BlendMode,
    pub thickness: u8,
    /// Index of a color in the menu color param, or 0 to use the custom color.
    pub palette_color: i32,
    /// Custom color used when palette_color is 0.
    pub custom_color: Color,
}

impl Default for MonoFrame {
    fn default() -> Self {
        Self { blend_mode: BlendMode::Alpha, thickness: 1, palette_color: 0, custom_color: Color::BLACK }
    }
}

impl MonoFrame {
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion) -> Result<Self, BinaryReaderError> {
        let blend_mode = BlendMode::from_u8(reader.read_u8()?)?;
        if version == DRBVersion::ACFA {
            reader.read_u16()?; // TODO: ACFA: What was this?
        } else {
            reader.assert::<u16>(0)?;
        }
        Ok(Self {
            blend_mode,
            thickness: reader.read_u8()?,
            palette_color: reader.read_i32()?,
            custom_color: Color::from_reader(reader)?,
        })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u8(self.blend_mode.to_u8())?;
        writer.write_u16(0)?; // TODO: ACFA
        writer.write_u8(self.thickness)?;
        writer.write_i32(self.palette_color)?;
        self.custom_color.to_writer(writer)
    }
}

///A rectangle with a solid color.
#[derive(Debug)]
pub struct MonoRect {
    pub blend_mode: BlendMode,
    /// Index of a color in the menu color param, or 0 to use the custom color.
    pub palette_color: i32,
    /// Custom color used when palette_color is 0.
    pub custom_color: Color,
}

impl Default for MonoRect {
    fn default() -> Self {
        Self { blend_mode: BlendMode::Alpha, palette_color: 0, custom_color: Color::BLACK }
    }
}

impl MonoRect {
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion) -> Result<Self, BinaryReaderError> {
        let blend_mode = BlendMode::from_u8(reader.read_u8()?)?;
        if version == DRBVersion::ACFA {
            // TODO: ACFA: What was this?
            reader.read_u16()?;
            reader.read_u8()?;
        } else {
            reader.assert::<u16>(0)?;
            reader.assert::<u8>(0)?;
        }
        Ok(Self {
            blend_mode,
            palette_color: reader.read_i32()?,
            custom_color: Color::from_reader(reader)?,
        })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u8(self.blend_mode.to_u8())?;
        // TODO: ACFA
        writer.write_u16(0)?;
        writer.write_u8(0)?;
        writer.write_i32(self.palette_color)?;
        self.custom_color.to_writer(writer)
    }
}


///Unknown structure only observed in Shadow Assault: Tenchu.
#[derive(Debug)]
pub struct TextUnknownB {
    pub unk00: i32,
    /// Always 0xFF.
    pub unk04: i16,
    /// Always 0 or 2.
    pub unk06: i16,
    /// Always 0 or 2.
    pub unk08: i16,
}

impl Default for TextUnknownB {
    fn default() -> Self {
        Self { unk00: 0, unk04: 0xFF, unk06: 0, unk08: 0 }
    }
}

impl IO for TextUnknownB {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self {
            unk00: reader.read_i32()?,
            unk04: reader.read_i16()?,
            unk06: reader.read_i16()?,
            unk08: reader.read_i16()?,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.unk00)?;
        writer.write_i16(self.unk04)?;
        writer.write_i16(self.unk06)?;
        writer.write_i16(self.unk08)
    }
}

///Unknown structure that is always present on text in practice.
#[derive(Debug, Default)]
pub struct TextUnknownA {
    /// Always 0.
    pub unk00: i32,
    pub sub_unknown: Option<TextUnknownB>,
}

impl IO for TextUnknownA {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        let unk00 = reader.read_i32()?;
        let unk04 = reader.read_i16()?;
        let sub_unknown = match unk04 {
            0 => None,
            1 => Some(TextUnknownB::from_reader(reader)?),
            other => return Err(format!("Expected 0 or 1 for TextUnknownA flag, got {}", other).into()),
        };
        Ok(Self { unk00, sub_unknown })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i32(self.unk00)?;
        writer.write_i16(self.sub_unknown.is_some() as i16)?;
        if let Some(mut sub) = self.sub_unknown.as_mut() {
            sub.to_writer(writer)?;
        }
        Ok(())
    }
}

///Extra fields that only ScrollText has
#[derive(Debug)]
pub struct ScrollParams {
    pub unk_x00: i16,
    pub unk_x02: i16,
    pub unk_x04: i16,
    pub unk_x06: i16,
    /// Always 15 or 100.
    pub scroll_speed: i16,
    pub unk_x0a: i16,
}

impl Default for ScrollParams {
    fn default() -> Self {
        Self { unk_x00: 0, unk_x02: 0, unk_x04: 0, unk_x06: 0, scroll_speed: 15, unk_x0a: 0 }
    }
}

impl IO for ScrollParams {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self {
            unk_x00: reader.read_i16()?,
            unk_x02: reader.read_i16()?,
            unk_x04: reader.read_i16()?,
            unk_x06: reader.read_i16()?,
            scroll_speed: reader.read_i16()?,
            unk_x0a: reader.read_i16()?,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i16(self.unk_x00)?;
        writer.write_i16(self.unk_x02)?;
        writer.write_i16(self.unk_x04)?;
        writer.write_i16(self.unk_x06)?;
        writer.write_i16(self.scroll_speed)?;
        writer.write_i16(self.unk_x0a)
    }
}


/// Either a fixed text or scrolling text element.
/// `scroll` is `Some` for ScrollText and `None` for Text.
#[derive(Debug)]
pub struct TextBase {
    pub blend_mode: BlendMode,
    /// Distance between each line of text.
    pub line_spacing: i16,
    /// Index of a color in the menu color param, or 0 to use the custom color.
    pub palette_color: i32,
    /// Custom color used when palette_color is 0.
    pub custom_color: Color,
    /// 0-11 are sizes of the menu font; 12 is the subtitle font.
    pub font_size: i16,
    pub alignment: AlignFlags,
    pub text_type: TxtType,
    /// Maximum characters to display (-1 for literals).
    pub char_length: i32,
    /// If `text_type` is Literal, the text to display.
    pub text_literal: Option<String>,
    text_offset: u32,
    /// If `text_type` is FMG, the FMG ID to display, otherwise -1.
    pub text_id: i32,
    pub unknown: Option<TextUnknownA>,
    pub scroll: Option<ScrollParams>,
}

impl Default for TextBase {
    fn default() -> Self {
        Self {
            blend_mode: BlendMode::Alpha,
            line_spacing: 0,
            palette_color: 0,
            custom_color: Color::BLACK,
            font_size: 0,
            alignment: AlignFlags::TOPLEFT,
            text_type: TxtType::FMG,
            char_length: 0,
            text_literal: None,
            text_offset: 0,
            text_id: -1,
            unknown: Some(TextUnknownA::default()),
            scroll: None,
        }
    }
}

impl TextBase {
    pub fn from_reader(reader: &mut BinaryReader, version: DRBVersion, is_scroll: bool) -> Result<Self, BinaryReaderError> {
        let blend_mode = BlendMode::from_u8(reader.read_u8()?)?;

        let mut unk01 = false;
        if version == DRBVersion::ACFA {
            reader.read_u8()?; // TODO: ACFA: What was this?
        } else {
            unk01 = reader.read_u8()? != 0;
        }

        let line_spacing = reader.read_i16()?;
        let palette_color = reader.read_i32()?;
        let custom_color = Color::from_reader(reader)?;
        let font_size = reader.read_i16()?;
        let alignment = AlignFlags::from_bits_retain(reader.read_u8()?);
        let text_type = TxtType::from_u8(reader.read_u8()?)?;
        reader.assert::<i32>(0x1C)?; // Local offset to variable data below

        let mut char_length = -1;
        let mut text_literal = None;
        let mut text_offset = 0;
        let mut text_id = -1;

        match text_type {
            TxtType::Literal => {
                text_offset = reader.read_u32()?;
                text_literal = Some(reader.get_utf16(text_offset as u64)?);
            }
            TxtType::FMG => {
                char_length = reader.read_i32()?;
                text_id = reader.read_i32()?;
            }
            TxtType::Dynamic => {
                char_length = reader.read_i32()?;
            }
        }

        let scroll = if is_scroll { Some(ScrollParams::from_reader(reader)?) } else { None };

        let unknown = if unk01 { Some(TextUnknownA::from_reader(reader)?) } else { None };

        Ok(Self {
            blend_mode,
            line_spacing,
            palette_color,
            custom_color,
            font_size,
            alignment,
            text_type,
            char_length,
            text_literal,
            text_offset,
            text_id,
            unknown,
            scroll,
        })
    }

    pub fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_u8(self.blend_mode.to_u8())?;
        writer.write_u8(self.unknown.is_some() as u8)?; // TODO: ACFA: Unk01
        writer.write_i16(self.line_spacing)?;
        writer.write_i32(self.palette_color)?;
        self.custom_color.to_writer(writer)?;
        writer.write_i16(self.font_size)?;
        writer.write_u8(self.alignment.bits())?;
        writer.write_u8(self.text_type.to_u8())?;
        writer.write_i32(0x1C)?;

        match self.text_type {
            TxtType::Literal => {
                writer.write_u32(self.text_offset)?;
            }
            TxtType::FMG => {
                writer.write_i32(self.char_length)?;
                writer.write_i32(self.text_id)?;
            }
            TxtType::Dynamic => {
                writer.write_i32(self.char_length)?;
            }
        }

        if let Some(mut scroll) = self.scroll.as_mut() {
            scroll.to_writer(writer)?;
        }

        if let Some(mut unknown) = self.unknown.as_mut() {
            unknown.to_writer(writer)?;
        }
        Ok(())
    }
}


/// Unknown.
#[derive(Debug)]
pub struct ScriptCtrlDialog {
    pub unk00: i16,
    /// Always 0.
    pub unk02: i16,
    /// Always 0.
    pub unk04: i32,
    /// Always 0xFF, may be RGBA.
    pub unk08: u8,
    /// Always 0xFF, may be RGBA.
    pub unk09: u8,
    /// Always 0xFF, may be RGBA.
    pub unk0a: u8,
    /// Always 0xFF, may be RGBA.
    pub unk0b: u8,
    pub unk0c: i32,
}

impl Default for ScriptCtrlDialog {
    fn default() -> Self {
        Self { unk00: 0, unk02: 0, unk04: 0, unk08: 0, unk09: 0, unk0a: 0, unk0b: 0, unk0c: 0 }
    }
}

impl IO for ScriptCtrlDialog {
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        Ok(Self {
            unk00: reader.read_i16()?,
            unk02: reader.read_i16()?,
            unk04: reader.read_i32()?,
            unk08: reader.read_u8()?,
            unk09: reader.read_u8()?,
            unk0a: reader.read_u8()?,
            unk0b: reader.read_u8()?,
            unk0c: reader.read_i32()?,
        })
    }

    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        writer.write_i16(self.unk00)?;
        writer.write_i16(self.unk02)?;
        writer.write_i32(self.unk04)?;
        writer.write_u8(self.unk08)?;
        writer.write_u8(self.unk09)?;
        writer.write_u8(self.unk0a)?;
        writer.write_u8(self.unk0b)?;
        writer.write_i32(self.unk0c)
    }
}

