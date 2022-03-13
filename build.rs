use ruc::*;
use vergen::{vergen, Config};

fn main() -> Result<()> {
    // Generate the default 'cargo:' instruction output
    vergen(Config::default()).map_err(|e| eg!(e.to_string()))
}
