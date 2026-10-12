use std::path::Path;

pub mod dxgi;
pub mod dds;
pub mod utils;
pub mod swizzling;
pub mod headerizer;
pub mod tpf;


///Dumps all icons in the binder by cropping them according to dimensions defined in the shoebox layout.
/// 
/// ## Note:
/// 
///As these DDS files are block-compressed, their dimensions must all be multiples of 4.
/// 
///Therefore, this code rounds all subtexture dimensions so that they are valid. These icons will not look
/// identical to as they are in game, especially if they are small. If you want them to be cropped correctly,
/// use `TPFTexture::to_dds::to_bytes` and then pass the bytes to an external image handling library and convert it to a PNG.
/// 
/// ## Safety:
/// 
/// This function loads BND4s, which are likely in KRAK format and thus need oo2core loaded. Safety cannot be guaranteed for 
/// an external library.
pub unsafe fn dump_all_icons(tpf_path: &Path, sblyt_path: &Path, output_dir: &Path) -> Result<(), crate::errors::FerrisoulsError> {
    use crate::{binary::IO, binders::{Binder, MetaBinder}};
    use crate::formats::sblyt::{Layout, LayoutBinder};
    use crate::textures::{dds::DDS, tpf::TPF};

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
                    let mut icon = DDS::from_bytes(
                        &dds.crop(
                            sub.x,
                            sub.y,
                            sub.width,
                            sub.height
                        )?
                    )?;
                    //write dds file to the parent folder
                    icon.to_file(&texture_dir.join(format!("{}.dds", sub.name.replace(".png", ""))))?;
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
            dump_all_icons(
                &Path::new("tests/01_common.tpf.dcx"),
                &Path::new("tests/01_common.sblytbnd.dcx"),
                &Path::new(r"C:\Users\lstr\Downloads\Textures")
            ).unwrap()
        }
    }
}