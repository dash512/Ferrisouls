use std::path::Path;
use std::fmt::Write;

use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::binders::{MetaBinder, MetaEntry};
use crate::binders::bnd4::{BND4Entry, BND4EntryHeader, BND4Header};
use crate::binders::{Binder, BinderEntry, BinderVersion, bnd4::BND4};
use crate::dcx::DCXType;
use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError};
use crate::games::Game;

use roxmltree::{Document, Node};
use xmlwriter::{Indent, Options, XmlWriter};


///Represents a single SubTexture entry in a layout XML
#[derive(Debug)]
pub struct Subtexture {
    name: String,

    x: u16,
    y: u16,
    width: u16,
    height: u16,

    original_width: Option<u16>,
    original_height: Option<u16>,

    flag_half: bool, //repr 0/1
}

impl BinderEntry for Subtexture {
    type Identifier = String;

    fn identity(&self) -> &Self::Identifier {
        &self.name
    }
}

impl Subtexture {
    pub fn from_node(node: Node) -> Result<Self, BinaryReaderError> {
        let name: String = parse_attr(node, "name")?;
        let x: u16 = parse_attr(node, "x")?;
        let y: u16 = parse_attr(node, "y")?;
        let width: u16 = parse_attr(node, "width")?;
        let height: u16 = parse_attr(node, "height")?;
        let flag_half: bool = parse_attr::<u8>(node, "half")? != 0;
        
        let original_width = node.attribute("originalWidth")
            .map(|x| x.trim()
                .parse::<u16>()
                .map_err(|e| BinaryReaderError::Custom(e.to_string())))
            .transpose()?;  

        let original_height: Option<u16> = node.attribute("originalHeight")
            .map(|x| x.trim()
                .parse::<u16>()
                .map_err(|e| BinaryReaderError::Custom(e.to_string())))
            .transpose()?;
        
        Ok(
            Self {
                name,
                x,
                y,
                width,
                height,
                original_width,
                original_height,
                flag_half: flag_half
            }
        )
    }

    pub fn write_xml(&self, w: &mut XmlWriter) {
        w.start_element("SubTexture");
        w.write_attribute("name", &self.name);
        w.write_attribute("x", &self.x);
        w.write_attribute("y", &self.y);
        w.write_attribute("width", &self.width);
        w.write_attribute("height", &self.height);

        if let Some(ow) = self.original_width {
            w.write_attribute("originalWidth", &ow);
        }
        if let Some(oh) = self.original_height {
            w.write_attribute("originalHeight", &oh);
        }

        w.write_attribute("half", &(self.flag_half as u8));
        w.end_element();
    }
}



///Represents a `.layout` file inside a BND4
/// 
///IMPORTANT:
/// 
///`Layout` does not properly implement `IO`, but the trait is required for `MetaEntry`'s defaults.
///This struct's `to_entry` and `from_entry` implement them differently, so `IO` isn't needed.
/// 
///Attempting to call any `IO` functions on this struct will panic with `"not implemented"`.
#[derive(Debug)]
pub struct Layout {
    _header: BND4EntryHeader, // used when repacking the layout

    name: Option<String>,
    image_path: String,

    width: Option<u16>,
    height: Option<u16>,
    root_dimensions: bool, // whether to write atlas dimensions to the TextureAtlas xml root; used in NR

    entries: Vec<Subtexture>
}

impl Binder for Layout {
    const VERSION: BinderVersion = BinderVersion::V4;
    type Entry = Subtexture;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}

impl BinderEntry for Layout {
    type Identifier = String;

    fn identity(&self) -> &Self::Identifier {
        &self.image_path
    }
}

impl Layout {
    fn from_bytes(name: Option<String>, header: BND4EntryHeader, data: &[u8]) -> Result<Self, BinaryReaderError> {
        let raw_xml = str::from_utf8(data)
            .map_err(|e| BinaryReaderError::Custom(e.to_string()))?;

        let doc = Document::parse(&raw_xml)
            .map_err(|e| BinaryReaderError::Custom(e.to_string()))?;

        let root = doc.root_element();

        if !root.has_tag_name("TextureAtlas") {
            return Err(BinaryReaderError::custom("root element is not <TextureAtlas>"));
        }

        let image_path = root.attribute("imagePath")
            .ok_or_else(|| BinaryReaderError::custom("TextureAtlas doesn't contain an imagePath."))?
            .to_string();

        let subs = root.children()
            .filter(|n| n.has_tag_name("SubTexture"))
            .map(Subtexture::from_node)
            .collect::<Result<Vec<_>, _>>()?;

        match root.attribute("width") {
            Some(w) => {
                let width = root.attribute("width")
                    .map(|w| w.trim().parse::<u16>().map_err(|e| BinaryReaderError::Custom(e.to_string())))
                    .transpose()?;
                let height = root.attribute("height")
                    .map(|h| h.trim().parse::<u16>().map_err(|e| BinaryReaderError::Custom(e.to_string())))
                    .transpose()?;
                let root_dimensions = width.is_some() || height.is_some();

                Ok(Self {
                    _header: header,
                    name,
                    image_path,
                    width,
                    height,
                    root_dimensions,
                    entries: subs,
                })
            },
            None => {
                Ok(Self {
                    _header: header,
                    name,
                    image_path,
                    width: None,
                    height: None,
                    root_dimensions: false,
                    entries: subs
                })
            }
        }

    }
}

impl IO for Layout {} // NOT IMPLEMENTED! Only used for MetaEntry

impl MetaEntry for Layout {
    fn header(&self) -> Option<BND4EntryHeader> {
        Some(self._header.clone())
    }

    fn name(&self) -> Option<String> {
        self.name.clone()
    }

    fn set_header(&mut self, header: &BND4EntryHeader) {
        self._header = header.clone()
    }

    fn set_name(&mut self, name: &Option<String>) {
        self.name = name.clone()
    }

    fn from_entry(entry: &mut BND4Entry) -> Result<Self, BinaryReaderError> {
        Self::from_bytes(entry.name.clone(), entry.header.clone(), &entry.data)
    }

    ///Recompiles self as xml data and returns a `BND4Entry`.
    fn to_entry(&mut self) -> Result<BND4Entry, BinaryWriterError> {
        let mut w = XmlWriter::new(Options {
            indent: Indent::Tabs,
            ..Options::default()
        });

        w.start_element("TextureAtlas");
        w.write_attribute("imagePath", &self.image_path);

        if self.root_dimensions {
            if let Some(width) = self.width {
                w.write_attribute("width", &width);
            }
            if let Some(height) = self.height {
                w.write_attribute("height", &height);
            }
        }

        for sub in &self.entries {
            sub.write_xml(&mut w);
        }

        let xml = w.end_document();

        Ok(BND4Entry {
            header: self._header.clone(),
            name: self.name.clone(),
            data: xml.into_bytes(),
        })
    }

}


///Essentially just a BND4, defined here for QoL
#[derive(Debug)]
pub struct LayoutBinder {
    header: BND4Header,
    entries: Vec<Layout>
}

impl Binder for LayoutBinder {
    const VERSION: BinderVersion = BinderVersion::V4;
    type Entry = Layout;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}

impl IO for LayoutBinder {
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

impl MetaBinder for LayoutBinder {
    fn header(&self) -> BND4Header {
        self.header.clone()
    }

    fn new(header: BND4Header, entries: Vec<<Self as Binder>::Entry>) -> Self {
        Self { header, entries }
    }
}

fn parse_attr<T>(node: roxmltree::Node, name: &str) -> Result<T, BinaryReaderError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{

    let attr = node.attribute(name).ok_or_else(|| {
        BinaryReaderError::custom(format!(
            "<{}> is missing attribute '{}'",
            node.tag_name().name(),
            name
        ))
    })?;

    attr.trim().parse::<T>().map_err(|e| {
        BinaryReaderError::custom(format!("invalid '{}' attribute: {}", name, e))
    })
}


pub fn get_image_path(game: Game, name: &str, resolution: &str) -> String {
    match game {
        Game::NR => format!(r"W:\CL\data\Target\INTERROOT_win64\menu\ScaleForm\Tif\01_Common\{resolution}\{name}.tif"),

        Game::AC6 => format!(r"W:\FNR\data\Menu\ScaleForm\Tif\01_Common\{name}\{resolution}\exp\{name}.png"),

        _=> format!("{name}.png") // SDT/ER
    }
}




#[cfg(test)]
mod tests {
    use crate::{dcx::{self, CompressSettings}, oodle::{core::init_oodle, structs::OodleSettings}};
    use super::*;

    #[test]
    fn full_layout_test() {
        unsafe { init_oodle(Path::new("tests/oo2core_6_win64.dll")) };

        let (mut lyts, dcx_type) = unsafe { LayoutBinder::unpack_binder(
            Path::new("tests/01_common.sblytbnd.dcx")
        ).unwrap() };

        for l in lyts.iter_mut() {
            for s in l.iter_mut() {
                s.name = String::from("fish");
            }
        }

        let mut binder = unsafe { lyts.pack_binder().unwrap() };
        unsafe {
        binder.to_file(
            Path::new(r"C:/Users/lstr/Downloads/test.sblytbnd.dcx"),
            CompressSettings::Oodle(dcx_type, OodleSettings::KRAK)
        ).unwrap();
        }
    }
}