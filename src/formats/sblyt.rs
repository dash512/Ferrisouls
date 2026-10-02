use crate::binders::{Binder, BinderEntry, BinderVersion, bnd4::BND4};
use crate::errors::FerrisoulsError;


pub struct Subtexture {
    name: String,

    x: u16,
    y: u16,
    width: u16,
    height: u16,

    original_width: u16,
    original_height: u16,

    flag_half: bool, //repr 0/1
}

impl BinderEntry for Subtexture {
    type Identifier = String;

    fn identity(&self) -> &Self::Identifier {
        &self.name
    }
}

pub struct Layout {
    name: String,
    root_dimensions: bool, // whether to write atlas dimensions to the TextureAtlas xml root; used in NR
    entries: Vec<Subtexture>
}

impl Layout {
    pub fn from_bnd(binder: BND4) -> Result<Vec<Self>, FerrisoulsError> {
        todo!()
    }
}

impl Binder for Layout {
    const VERSION: BinderVersion = BinderVersion::V4;
    type Entry = Subtexture;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}

