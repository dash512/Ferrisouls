//! This module contains some submodules for handling anything to do with textures,
//! as well as some example functions for their uses in 'the field'.

use std::{io::Write, path::Path};

use crate::errors::FerrisoulsError;


pub mod dxgi;
pub mod dds;
pub mod utils;
pub mod swizzling;
pub mod headerizer;
pub mod image;



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
pub unsafe fn dump_icons_dds(tpf_path: &Path, sblyt_path: &Path, output_dir: &Path) -> Result<(), crate::errors::FerrisoulsError> {
    use crate::{binary::IO, binders::{Binder, MetaBinder}};
    use crate::formats::{tpf::TPF, sblyt::{Layout, LayoutBinder}};
    use crate::textures::dds::DDS;

    //try to automatically find and load oo2core
    unsafe { crate::oodle::auto_oodle().unwrap(); }

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


///Exports all Atlases in a given TPF to the output directory as `.dds` files.
/// 
/// ## Safety:
/// 
/// This function loads BND4s, which are likely in KRAK format and thus need oo2core loaded. Safety cannot be guaranteed for 
/// an external library.
pub unsafe fn dump_atlases(tpf_path: &Path, output_dir: &Path) -> Result<(), crate::errors::FerrisoulsError> {
    use crate::{binary::IO, binders::Binder};
    use crate::formats::tpf::TPF;
    use crate::textures::image::ImageFormat;

    unsafe { crate::oodle::auto_oodle().unwrap(); }

    std::fs::create_dir_all(output_dir)?;
    let (tpf, _) = unsafe { TPF::unpack(tpf_path)? };

    for tex in tpf.into_iter() {
        tex.to_dds()?.to_file(&output_dir.join(tex.name.clone()).with_extension("dds"))?;
    }
    Ok(())
}


///Dumps all Atlas images from any TPF binders found in the given `menu_path` directory. Images are encoded to TGA format.
/// 
///If `do_sprites` is true, then any layouts found in the same directory are used to crop the loaded Atlases
/// and write all of their subtextures.
/// 
///Mainly useful for viewing images in JPEXS flash decompiler.
/// 
/// ## Safety:
/// 
/// This function loads BND4s, which are likely in KRAK format and thus need oo2core loaded. Safety cannot be guaranteed for 
/// an external library.
pub unsafe fn dump_tgas(menu_path: &Path, output_dir: &Path, do_sprites: bool) -> Result<(), crate::errors::FerrisoulsError> {
    use crate::{binary::IO, binders::{Binder, MetaBinder}};
    use crate::formats::{tpf::{TPF, TPFTexture}, sblyt::{Layout, LayoutBinder}};
    use crate::textures::image::{self, ImageFormat};
    use crate::files;

    unsafe { crate::oodle::auto_oodle().unwrap(); }

    std::fs::create_dir_all(output_dir)?;

    let tpfs = files::get_files_by(
        menu_path.to_path_buf(), 
        |s| s.ends_with(".tpf") || s.ends_with(".tpf.dcx")
    );
    let sblyts = files::get_files_by(
        menu_path.to_path_buf(), 
        |s| s.contains(".sblytbnd")
    );

    let mut textures: Vec<TPFTexture> = Vec::new();
    let mut layouts: Vec<Layout> = Vec::new();

    {
        for tpf_path in tpfs {
            let (tpf, _) = unsafe { TPF::unpack(&tpf_path)? };
            println!("Unpacked TPF: {:?} with {} entries", tpf_path.file_name(), tpf.count());
            textures.extend(tpf.textures);
        }

        for sblyt_path in sblyts {
            let (sblyt, _) = unsafe { LayoutBinder::unpack_binder(&sblyt_path)? };
            println!("Unpacked Layout Binder: {:?} with {} entries", sblyt_path.file_name(), sblyt.count());
            layouts.extend(sblyt.entries);
        }
    }

    for tex in textures {
        let tga = tex.to(ImageFormat::TGA)?;
        std::fs::write(
            output_dir.join(format!("{}.tga", tex.name)),
            &tga,
        )?;
        println!("Wrote TPFTexture: {:?}.", tex.name);

        if !do_sprites {
            continue;
        }

        let layout = layouts
            .iter()
            .find(|l| l.stem() == tex.name);

        if let Some(layout) = layout {
            println!("Found layout for {}! Cropping {} subtextures...", tex.name, layout.entries.len());

            for sub in layout.iter() {
                let sprite = image::crop_tga(
                    &tga, 
                    sub.x.into(), 
                    sub.y.into(), 
                    sub.width.into(), 
                    sub.height.into()
                );
                match sprite {
                    None => {
                        println!("Failed to crop tga `{}`, skipping!", tex.name);
                        continue;
                    },
                    Some(s) => {
                        std::fs::write(
                            output_dir.join(format!("{}.tga", sub.name.replace(".png", ""))),
                            &s,
                        )?;
                    }
                }
            }
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
            dump_icons_dds(
                &Path::new("tests/01_common.tpf.dcx"),
                &Path::new("tests/01_common.sblytbnd.dcx"),
                &Path::new(r"C:\Users\lstr\Downloads\Textures")
            ).unwrap()
        }
    }
}
