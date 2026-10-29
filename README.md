<p align="center">
  <img src="res/Assets/icon.png"/>
</p>

<p align="center">
<a href="#features">Features</a> /|\ <a href="#supported-formats">Formats</a> /|\ <a href="#acknowledgements">Credits</a>
</p>
  
## Features:
 - Oodle implementation for decompressing files (supports versions 6, 8 and 9)
 - DCX header reading and type detection
 - WIP swizzle/deswizzle functionality for console textures
    - Decently optimized morton/z-order algorithms for 8x8 tiles
 - DXGI enums with important info and descriptions
 - Basic binary readers and writers
 - Regulation decryption
  
  
## Supported Formats:
### Legend:
 - ✅ - Supported
 - ⚠️ - Under Construction/Planned
 - ❌ - Not Supported Yet
 - ⁉️ - What the hell is that/I need to do research before adding this
  
| Format | Supported |
|---|---|
| ACB | ⁉️ |
| BHD5 | ⚠️ |
| BND3 | ✅ |
| BND4 | ✅ |
| BTAB | ⁉️ |
| BTL | ⁉️ |
| BTPB | ⁉️ |
| BXF3 | ✅ |
| BXF4 | ✅ |
| CCM | ❌ |
| CLM2 | ⁉️ |
| DCX | ✅ |
| DRB | ⚠️ |
| EDD | ⁉️ |
| EDGE | ❌ |
| EMELD | ❌ |
| EMEVD | ⚠️ |
| ENFL | ⁉️ |
| ESD | ⚠️ |
| F2TR | ⁉️ |
| FFXDLSE | ❌ |
| FLVER | ⚠️ |
| FMG | ✅ |
| FXR3 | ⚠️ |
| GPARAM | ❌ |
| GRASS | ❌ |
| LUAGNL | ❌ |
| LUAINFO | ❌ |
| MCG | ⁉️ |
| MCP | ⁉️ |
| MSB | ⚠️ |
| MTD | ⚠️ |
| NGP | ⁉️ |
| NVA | ⚠️ |
| NVM | ❌ |
| PARAM | ✅ |
| PARAMDEF | ✅ |
| PARAMTDF | ⚠️ |
| PMDCL | ⁉️ |
| RMB | ⚠️ |
| TAE3 | ⚠️ |
| TPF | ✅ |
  
*For more information, see [FORMATS](https://github.com/soulsmods/SoulsFormatsNEXT/blob/master/FORMATS.md)


## Acknowledgements:
This project has been incredibly fun to work on, and NONE of it would have been possible if I weren't standing on the 
backs of giants. The dedication of people much smarter than myself to the modding community is what makes ANY modding even possible.  
Huge thanks and respects to everyone who worked on these projects:  
 - [SoulsFormats](https://github.com/JKAnderson/SoulsFormats) - Essentially the building blocks modding was based on.
 - [SoulsFormatsNEXT](https://github.com/soulsmods/SoulsFormatsNEXT) - The community continuation of the aforementioned project. Essentially all code in this repo is at least partially based off this.
 - [soulstruct](https://github.com/Grimrukh/soulstruct) - A python library for game modification. Inspired this project as well as my interest in tooling in general :)
 - [fstools-rs](https://github.com/soulsmods/fstools-rs) - An alternative to this project. No code was used but I want to show appreciation for it regardless.
 - [Paramdex](https://github.com/soulsmods/Paramdex) - Assorted data required to read Params.
 - [UXM](https://github.com/JKAnderson/UXM) + [Nuxe](https://github.com/JKAnderson/Nuxe) - Game data unpackers that are required before modding is even possible.
 - [Oodle](https://www.radgametools.com/oodle.htm) - A compression codec used in all modern games, created by RAD Game Tools. The library is not redistributed in this repo, and is required by the user to be loaded at runtime.


## Licensing
This project is licensed under [GPL-3.0](https://github.com/dash512/Ferrisouls/blob/master/LICENSE) due to how reliant it is on SoulsFormatsNEXT.
