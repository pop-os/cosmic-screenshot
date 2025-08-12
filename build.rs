// SPDX-License-Identifier: GPL-3.0-only

use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    EmitBuilder::builder()
        .all_git()
        .emit()?;
    Ok(())
}