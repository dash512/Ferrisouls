//! Groups of UI elements (`Dlg`) and the individual elements within them (`Dlgo`).

use std::collections::{HashMap, VecDeque};
use std::ops::{Deref, DerefMut};

use crate::binders::{Binder, BinderEntry};
use crate::{binary::{BinaryReader, BinaryWriter}, errors::{BinaryReaderError, BinaryWriterError}};
use super::{control::Control, shape::Shape};

pub const DLGO_SIZE: u32 = 0x20;

///An individual UI element.
#[derive(Debug, Default)]
pub struct Dlgo {
    /// The name of the element.
    pub name: String,
    /// Offset of the name in the STR block. Kept so the string table can be written back verbatim.
    pub name_offset: u32,
    /// The visual properties of the element.
    pub shape: Shape,
    /// The behavior of the element.
    pub control: Control,

    pub unk0c: i32, // 0
    pub unk10: i32, // 0
    pub unk14: i32, // 0
    pub unk18: i32, // 0
    pub unk1c: i32, // 0
}

impl Dlgo {
    /// `shapes` and `controls` are keyed by block-relative offset. Entries are removed as they're claimed.
    pub fn from_reader(reader: &mut BinaryReader, shapes: &mut HashMap<u32, Shape>, controls: &mut HashMap<u32, Control>) -> Result<Self, BinaryReaderError> {
        let name_offset = reader.read_u32()?;
        let shap_offset = reader.read_u32()?;
        let ctrl_offset = reader.read_u32()?;
        let unk0c = reader.read_i32()?;
        let unk10 = reader.read_i32()?;
        let unk14 = reader.read_i32()?;
        let unk18 = reader.read_i32()?;
        let unk1c = reader.read_i32()?;

        let name = reader.get_utf16(name_offset as u64)?;
        let shape = shapes
            .remove(&shap_offset)
            .ok_or_else(|| format!("No shape at SHAP offset {shap_offset:#X} for \"{name}\""))?;
        let control = controls
            .remove(&ctrl_offset)
            .ok_or_else(|| format!("No control at CTRL offset {ctrl_offset:#X} for \"{name}\""))?;

        Ok(Self { 
            name, 
            name_offset, 
            shape, 
            control, 
            unk0c, 
            unk10, 
            unk14, 
            unk18, 
            unk1c 
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, shap_offsets: &mut VecDeque<u32>, ctrl_offsets: &mut VecDeque<u32>) -> Result<(), BinaryWriterError> {
        writer.write_u32(self.name_offset)?;
        writer.write_u32(shap_offsets.pop_front().unwrap_or_default())?;
        writer.write_u32(ctrl_offsets.pop_front().unwrap_or_default())?;
        writer.write_i32(self.unk0c)?;
        writer.write_i32(self.unk10)?;
        writer.write_i32(self.unk14)?;
        writer.write_i32(self.unk18)?;
        writer.write_i32(self.unk1c)
    }
}

impl BinderEntry for Dlgo {
    type Identifier = String;
    fn identity(&self) -> &Self::Identifier { &self.name }
}


///A group of UI elements, which is itself a UI element.
#[derive(Debug)]
pub struct Dlg {
    /// The group's own name, shape, and control.
    pub base: Dlgo,
    /// The child elements attached to this group.
    pub dlgos: Vec<Dlgo>,

    pub left_edge: i16,
    pub top_edge: i16,
    pub right_edge: i16,
    pub bottom_edge: i16,

    pub unk30: [i16; 5],
    /// Always 0.
    pub unk3a: i16,
    /// Always 0.
    pub unk3c: i32,
}

impl Binder for Dlg {
    type Entry = Dlgo;
    
    fn entries(&self) -> &Vec<Self::Entry> { &self.dlgos }
    fn entries_mut(&mut self) -> &mut Vec<Self::Entry> { &mut self.dlgos }
}

impl Default for Dlg {
    fn default() -> Self {
        Self {
            base: Dlgo::default(),
            dlgos: Vec::new(),
            left_edge: 0,
            top_edge: 0,
            right_edge: 0,
            bottom_edge: 0,
            unk30: [-1; 5],
            unk3a: 0,
            unk3c: 0,
        }
    }
}

impl Deref for Dlg {
    type Target = Dlgo;
    fn deref(&self) -> &Dlgo { &self.base }
}

impl DerefMut for Dlg {
    fn deref_mut(&mut self) -> &mut Dlgo { &mut self.base }
}

impl Dlg {
    pub fn new(name: String, name_offset: u32, shape: Shape, control: Control) -> Self {
        Self {
            base: Dlgo { name, name_offset, shape, control, ..Default::default() },
            ..Default::default()
        }
    }

    ///`dlgos` is keyed by block-relative offset. Children are removed as they're claimed.
    pub fn from_reader(reader: &mut BinaryReader, shapes: &mut HashMap<u32, Shape>, controls: &mut HashMap<u32, Control>, dlgos: &mut HashMap<u32, Dlgo>) -> Result<Self, BinaryReaderError> {
        let base = Dlgo::from_reader(reader, shapes, controls)?;

        let dlgo_count = reader.read_i32()?;
        let dlgo_offset = reader.read_i32()?;
        let left_edge = reader.read_i16()?;
        let top_edge = reader.read_i16()?;
        let right_edge = reader.read_i16()?;
        let bottom_edge = reader.read_i16()?;

        let mut unk30 = [0i16; 5];
        for v in &mut unk30 {
            *v = reader.read_i16()?;
        }
        let unk3a = reader.read_i16()?;
        let unk3c = reader.read_i32()?;

        let mut children = Vec::with_capacity(dlgo_count.max(0) as usize);
        for i in 0..dlgo_count.max(0) as u32 {
            let offset = dlgo_offset as u32 + DLGO_SIZE * i;
            children.push(
                dlgos
                    .remove(&offset)
                    .ok_or_else(|| format!("No DLGO at offset {offset:#X} for \"{}\"", base.name))?,
            );
        }

        Ok(Self {
            base,
            dlgos: children,
            left_edge,
            top_edge,
            right_edge,
            bottom_edge,
            unk30,
            unk3a,
            unk3c,
        })
    }

    pub fn to_writer(&self, writer: &mut BinaryWriter, shap_offsets: &mut VecDeque<u32>, ctrl_offsets: &mut VecDeque<u32>, dlgo_offsets: &mut VecDeque<u32>) -> Result<(), BinaryWriterError> {
        self.base.to_writer(writer, shap_offsets, ctrl_offsets)?;
        writer.write_i32(self.dlgos.len() as i32)?;
        writer.write_i32(dlgo_offsets.pop_front().unwrap_or_default() as i32)?;
        writer.write_i16(self.left_edge)?;
        writer.write_i16(self.top_edge)?;
        writer.write_i16(self.right_edge)?;
        writer.write_i16(self.bottom_edge)?;
        for v in self.unk30 {
            writer.write_i16(v)?;
        }
        writer.write_i16(self.unk3a)?;
        writer.write_i32(self.unk3c)
    }

    ///Returns the child element with the given name.
    pub fn get(&self, name: &str) -> Option<&Dlgo> {
        self.dlgos.iter().find(|d| d.name == name)
    }
}

