use std::path::Path;

use crate::binary::{BinaryReader, BinaryWriter, IO};
use crate::binders::bnd4::BND4Entry;
use crate::binders::{Binder, BinderEntry, BinderVersion, bnd4::BND4};
use crate::errors::{BinaryReaderError, BinaryWriterError, FerrisoulsError};

use roxmltree::{Document, Node};


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
    pub fn from_node(node: Node) -> Result<Self, FerrisoulsError> {
        let name: String = parse_attr(node, "name")?;
        let x: u16 = parse_attr(node, "x")?;
        let y: u16 = parse_attr(node, "y")?;
        let width: u16 = parse_attr(node, "width")?;
        let height: u16 = parse_attr(node, "height")?;
        let flag_half: bool = parse_attr::<u8>(node, "half")? != 0;
        
        let original_width = node.attribute("originalWidth")
            .map(|x| x.trim()
                .parse::<u16>()
                .map_err(|e| FerrisoulsError::custom(e.to_string())))
            .transpose()?;  

        let original_height: Option<u16> = node.attribute("originalHeight")
            .map(|x| x.trim()
                .parse::<u16>()
                .map_err(|e| FerrisoulsError::custom(e.to_string())))
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
}



#[derive(Debug)]
pub struct Layout {
    name: Option<String>,
    image_path: String,

    width: Option<u16>,
    height: Option<u16>,
    root_dimensions: bool, // whether to write atlas dimensions to the TextureAtlas xml root; used in NR

    entries: Vec<Subtexture>
}

impl IO for Layout {
    ///Unimplemented
    fn from_reader(reader: &mut BinaryReader) -> Result<Self, BinaryReaderError> {
        unimplemented!()
    }

    ///Unimplemented
    fn to_writer(&mut self, writer: &mut BinaryWriter) -> Result<(), BinaryWriterError> {
        unimplemented!()
    }
}

impl Binder for Layout {
    const VERSION: BinderVersion = BinderVersion::V4;
    type Entry = Subtexture;

    fn entries(&mut self) -> &mut Vec<Self::Entry> {
        &mut self.entries
    }
}

impl Layout {
    //Read
    pub fn from_entry(entry: &mut BND4Entry) -> Result<Self, FerrisoulsError> {
        let raw_xml = str::from_utf8(&entry.data)
            .map_err(|e| FerrisoulsError::custom(e.to_string()))?;

        let doc = Document::parse(&raw_xml)
            .map_err(|e| FerrisoulsError::custom(e.to_string()))?;

        let root = doc.root_element();

        if !root.has_tag_name("TextureAtlas") {
            return Err(FerrisoulsError::custom("root element is not <TextureAtlas>"));
        }

        let image_path = root.attribute("imagePath")
            .ok_or_else(|| FerrisoulsError::custom("TextureAtlas doesn't contain an imagePath."))?
            .to_string();

        let subs = root.children()
            .filter(|n| n.has_tag_name("SubTexture"))
            .map(Subtexture::from_node)
            .collect::<Result<Vec<_>, _>>()?;

        match root.attribute("width") {
            Some(w) => {
                let width = Some(
                        w
                        .trim()
                        .parse::<u16>()
                        .map_err(|e| FerrisoulsError::custom(e.to_string()))
                    ).transpose()?;

                let height = root.attribute("height")
                    .and_then(|i| Some(
                        i
                        .trim()
                        .parse::<u16>()
                        .map_err(|e| FerrisoulsError::custom(e.to_string()))
                    ))
                    .transpose()?;

                Ok(Self {
                    name: entry.name.clone(),
                    image_path,
                    width,
                    height,
                    root_dimensions: true,
                    entries: subs,
                })
            },
            None => {
                Ok(Self {
                    name: entry.name.clone(),
                    image_path,
                    width: None,
                    height: None,
                    root_dimensions: false,
                    entries: subs
                })
            }
        }

    }

    pub fn from_binder(binder: &mut BND4) -> Result<Vec<Self>, FerrisoulsError> {
        binder.iter()
            .map(Self::from_entry)
            .collect()
    }

    pub unsafe fn unpack(path: &Path) -> Result<Vec<Self>, FerrisoulsError> {
        let (mut binder, dcxtype) = unsafe { BND4::unpack(path)?};
        Self::from_binder(&mut binder)
    }

    //Write
}




fn parse_attr<T>(node: roxmltree::Node, name: &str) -> Result<T, FerrisoulsError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{

    let attr = node.attribute(name).ok_or_else(|| {
        FerrisoulsError::custom(format!(
            "<{}> is missing attribute '{}'",
            node.tag_name().name(),
            name
        ))
    })?;

    attr.trim().parse::<T>().map_err(|e| {
        FerrisoulsError::custom(format!("invalid '{}' attribute: {}", name, e))
    })
}



#[cfg(test)]
mod tests {
    use crate::oodle::core::init_oodle;

use super::*;

    #[test]
    fn parse_lyt() {
        unsafe { init_oodle(Path::new(r".../oo2core_6_win64.dll")) };
        let lyts = unsafe { Layout::unpack(
            Path::new(r".../tests/01_common_l.sblytbnd.dcx")
        ).unwrap() };

        for l in lyts {
            println!("{:#?}", l)
        }
    }
}