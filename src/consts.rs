mod attenuation;
mod time_bases;
mod vscale;

pub use attenuation::PROBE_ATTENUATIONS;
pub use time_bases::TIME_BASES;
pub use vscale::VERTICAL_SCALES;

/// Grid division size
pub const GRID_DIV_SIZE_INT: i64 = 25;
pub const GRID_DIV_SIZE: f64 = 25.0;
pub const GRID_DIV_COUNT_HORIZONTAL: f64 = 12.0;
pub const SAMPLES: usize = 300;
