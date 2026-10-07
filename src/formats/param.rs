use crate::errors::FerrisoulsError;

pub mod param;
pub mod cell;
pub mod row;
pub mod paramdef;
pub mod util;

pub struct ParamdefMatchOptions {
    pub min_score: u8,
    pub version: Option<u64>,
    pub require_version_aware: bool,
}

