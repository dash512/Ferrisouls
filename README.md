Uhh hi. Good job finding this I guess. This is essentially a very shitty Rust port of Soulstruct. With a very shitty name (ideas pls???)  
  
It is currently nowhere near finished or even really usable on its own, however, I decided to keep it public on the tiny off chance someone finds it and has some use of whatever's in here.  
  
I'm mostly doing this as a way to learn Rust through actual practice. Credit ofc to Grimrukh for Soulstruct, where a lot of this logic comes from, and the SoulsFormatsNext project.  
  
Soo yh that's about it I'm not gonna get a proper readme going until this library is at least servicable.  
This project is under GPL3 because Soulstruct is, even if there's no actual Soulstruct code here.
  
## Currently includes:
 - Oodle implementation for decompressing files (supports versions 6, 8 and 9)
 - DCX header reading and type detection
 - WIP swizzle/deswizzle functionality for console textures
    - Decently optimized morton/z-order algorithms for 8x8 tiles
 - DXGI enums with important info and descriptions
 - Basic binary readers and writers
 - Regulation decryption (not fully parsed as of now)
 - Basic outline for TPF handling, not ready for use yet
 - Raw parsing and handling for FMGs
 - Basic parsing for all major non-split binder versions
