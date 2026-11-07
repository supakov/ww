//! The base of WW (W#): what the framework and the programs on it share about files. Strict JSON
//! files with size, depth and character limits and the trait `Document` (`jsonfile`), the files
//! inside the program folder kept relative to it (`paths`), the clock (`clock`). No window here.

pub mod clock;
pub mod jsonfile;
pub mod paths;
