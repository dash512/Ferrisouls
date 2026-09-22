

pub mod shared {
    #[repr(i32)]
    pub enum CheckCRC {
        No = 0,
        Yes = 1,
        Force32 = 0x40000000
    }

    #[derive(Debug, Clone, Copy)]
    #[repr(i32)]
    pub enum CompressionLevel {
        Null = 0,
        SuperFast = 1,
        VeryFast = 2,
        Fast = 3,
        Normal = 4,

        Optimal1 = 5,
        Optimal2 = 6,
        Optimal3 = 7,
        Optimal4 = 8,
        Optimal5 = 9,

        HyperFast1 = -1,
        HyperFast2 = -2,
        HyperFast3 = -3,
        HyperFast4 = -4,

        Force32 = 0x40000000,
    }

    impl CompressionLevel {
        pub const HYPERFAST: CompressionLevel = Self::HyperFast1;
        pub const OPTIMAL: CompressionLevel = Self::Optimal2;
        pub const MAX: CompressionLevel = Self::Optimal5;
        pub const MIN: CompressionLevel = Self::HyperFast4;
        pub const INVALID: CompressionLevel = Self::Force32;
    }

    #[repr(i32)]
    pub enum DecodeThreadPhase {
        ThreadPhase1 = 1,
        ThreadPhase2 = 2,
        ThreadPhaseAll = 3,
    }

    impl DecodeThreadPhase {
        pub const UNTHREADED: DecodeThreadPhase = Self::ThreadPhaseAll;
    }

    #[repr(i32)]
    pub enum FuzzSafe {
        No = 0, // deprecated in 2.9; all are fuzz-safe
        Yes = 1
    }

    #[repr(i32)]
    pub enum Profile {
        Main = 0,
        Reduced = 1,
        Force32 = 0x40000000
    }

    #[repr(C)]
    pub enum Verbosity {
        Null = 0,
        Minimal = 1,
        Some = 2,
        Lots = 3,
        Force32 = 0x40000000
    }
}


pub mod o26 {
    pub use super::shared::*;

    #[derive(Debug, Clone, Copy)]
    #[repr(i32)]
    pub enum Compressor {
        Invalid = -1,
        Null = 3,

        Kraken = 8,
        Leviathan = 13,
        Mermaid = 9,
        Selkie = 11,
        Hydra = 12,

        BitKnit = 10,
        LZB16 = 4,
        LZNA = 7,
        LZH = 0,
        LZHLW = 1,
        LZNIB = 2,
        LZBLW = 5,
        LZA = 6,

        Count = 14,
        Force32 = 0x40000000,
    }
}

pub mod o28 {
    pub use super::shared::*;

    #[derive(Debug, Clone, Copy)]
    #[repr(i32)]
    pub enum Compressor {
        Invalid = -1,
        Null = 3,

        Kraken = 8,
        Leviathan = 13,
        Mermaid = 9,
        Selkie = 11,
        Hydra = 12,

        BitKnit = 10,
        LZB16 = 4,
        LZNA = 7,
        LZH = 0,
        LZHLW = 1,
        LZNIB = 2,
        LZBLW = 5,
        LZA = 6,

        Count = 14,
        Force32 = 0x40000000,
    }

    pub enum Jobify {
        Default = 0,
        Disable = 1,
        Normal = 2,
        Aggressive = 3,
        Count = 4
    }
    
}

pub mod o29 {
    pub use super::shared::*;

    #[derive(Debug, Clone, Copy)]
    #[repr(i32)]
    pub enum Compressor {
        Invalid = -1,
        Null = 3,

        Kraken = 8,
        Leviathan = 13,
        Mermaid = 9,
        Selkie = 11,
        Hydra = 12,

        LZB16 = 4,
        //old codecs deprecated

        Count = 14,
        Force32 = 0x40000000,
    }

    pub enum Jobify {
        Default = 0,
        Disable = 1,
        Normal = 2,
        Aggressive = 3,
        Count = 4
    }
    
}


