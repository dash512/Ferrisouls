use std::{io::Write, path::Path};

use crate::errors::FerrisoulsError;
pub use crate::formats::{tpf, sblyt};

pub mod dxgi;
pub mod dds;
pub mod utils;
pub mod swizzling;
pub mod headerizer;


///Dumps all icons in the binder by cropping them according to dimensions defined in the shoebox layout.
/// 
/// ## Note:
/// 
///As these DDS files are block-compressed, their dimensions must all be multiples of 4.
/// 
///Therefore, this code rounds all subtexture dimensions so that they are valid. Some of these icons will not look
/// identical to how they are in game, especially if they are small. If you want them to be cropped correctly,
/// handle the raw bytes from `TPFTexture::to_dds::to_bytes` yourself with custom conversion and editing logic.
/// 
///Alternatively, call `TPFTexture::to_png` and then manipulate the raw bytes in place with a library of your choice.
/// 
///Item icons are generally aligned to 4x4 so if you're dumping specifically for those (as you're likely to), they should look fine.
/// 
/// ## Safety:
/// 
/// This function loads BND4s, which are likely in KRAK format and thus need oo2core loaded. Safety cannot be guaranteed for 
/// an external library.
pub unsafe fn dump_icons(tpf_path: &Path, sblyt_path: &Path, output_dir: &Path) -> Result<(), crate::errors::FerrisoulsError> {
    use crate::{binary::IO, binders::{Binder, MetaBinder}};
    use crate::formats::{tpf::TPF, sblyt::{Layout, LayoutBinder}};
    use crate::textures::dds::DDS;

    //try to automatically find and load oo2core
    unsafe { crate::oodle::core::oodle_auto().unwrap(); }

    //create output location if it doesn't xist
    std::fs::create_dir_all(output_dir)?;

    //Decompress and unpack binders
    let (mut sblyt, _) = unsafe { LayoutBinder::unpack_binder(sblyt_path)? };
    let (tpf, _) = unsafe { TPF::unpack(tpf_path)? };

    for tex in tpf.into_iter() {
        let mut dds = tex.to_dds()?;

        //find a layout matching the texture name. use l.stem because e.g:
        //tex.name = SB_Boss
        //l.name = N:\\NTC\\data\\Menu\\ScaleForm\\SBLayout\\01_Common\\hi\\SB_Boss.layout
        let layout: Option<&mut Layout> = sblyt
            .iter_mut()
            .find(|l| l.stem() == tex.name);

        match layout {
            None => {
                //no layout was found, write standard texture 
                dds.to_file(&output_dir.join(format!("{}.dds", tex.name)))?
            },

            Some(mut layout) => {
                //layout was found, create a new folder with the same name as the atlas
                let texture_dir = output_dir.join(&tex.name);
                std::fs::create_dir_all(&texture_dir)?;

                //write each icon by cropping it from the parent texture
                for sub in layout.iter() {
                    //round all attribute to the nearest value divisble by 4
                    //we do this because BC formats are 4x4 texels
                    let sub = &sub.round_to(4);
                    //some textures like menu lines have 0 height and can't be divided by 4
                    if !(sub.width > 0 && sub.height > 0) {
                        eprintln!("Skipping file with invalid dimensions:\n{:?}", sub);
                        continue;
                    }

                    //create new dds using cropped bytes from the original
                    let mut icon = dds.crop(sub.x, sub.y, sub.width, sub.height)?;
                    //write dds file to the parent folder
                    icon.to_file(&texture_dir.join(format!("{}.dds", sub.name.replace(".png", ""))))?;
                }
            }
        }
    }
    Ok(())
}

///Exports all Atlases in a given TPF to the output directory as `.tga` files.
/// 
/// ## Safety:
/// 
/// This function loads BND4s, which are likely in KRAK format and thus need oo2core loaded. Safety cannot be guaranteed for 
/// an external library.
pub unsafe fn dump_atlases(tpf_path: &Path, output_dir: &Path) -> Result<(), crate::errors::FerrisoulsError> {
    use crate::{binary::IO, binders::Binder};
    use crate::formats::tpf::TPF;

    unsafe { crate::oodle::core::oodle_auto().unwrap(); }

    std::fs::create_dir_all(output_dir)?;
    let (tpf, _) = unsafe { TPF::unpack(tpf_path)? };

    for tex in tpf.into_iter() {
        tex.save_as(&output_dir.join(tex.name.clone()).with_extension("tga"), dds::ImageExportFormat::TGA);
    }
    Ok(())
}


///Dump all Atlases from all TPFs in a given directory. Encodes to TGA format.
/// 
///Mainly useful for viewing images in JPEXS flash decompiler.
pub unsafe fn dump_tgas(menu_path: &Path, output_dir: &Path) -> Result<(), crate::errors::FerrisoulsError> {
    use crate::{binary::IO, binders::{Binder, MetaBinder}};
    use crate::formats::{tpf::TPF, sblyt::{Layout, LayoutBinder}};
    use crate::textures::dds::DDS;

    unsafe { crate::oodle::core::oodle_auto().unwrap(); }
    std::fs::create_dir_all(output_dir)?;

    let mut tpfs = Vec::new();

    for f in menu_path.read_dir().expect("failed to read directory!") {
        if let Ok(l) = f 
            && l.path().to_str().unwrap().contains(".tpf.") 
        {
            tpfs.push(l.path());
        }
    }

    for tpf_path in tpfs {
        let (mut tpf, _) = unsafe { TPF::unpack(&tpf_path)? };

        for t in tpf.iter() {
            std::fs::write(
                output_dir.join(format!("{}.tga", t.name)),
                t.to_tga()?,
            )?;
        }
    }

    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dumper() {
        unsafe {
            dump_atlases(
                &Path::new("tests/common.tpf.dcx"),
                &Path::new(r"C:\Users\lstr\Downloads\Textures")
            ).unwrap()
        }
    }

    #[test]
    fn test_icon_dumper() {
        unsafe {
            dump_icons(
                &Path::new("tests/01_common.tpf.dcx"),
                &Path::new("tests/01_common.sblytbnd.dcx"),
                &Path::new(r"C:\Users\lstr\Downloads\Textures")
            ).unwrap()
        }
    }
}
