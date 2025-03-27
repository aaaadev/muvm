use std::fs::File;
use std::io::Write;

use anyhow::{anyhow, Context, Result};

use crate::utils::env::find_in_path;

const ROSETTA_BINFMT_MISC_RULE: &str =
    ":rosetta:M::\\x7fELF\\x02\\x01\\x01\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x02\\x00\\x3e\\x00:\\xff\\xff\\xff\\xff\\xff\\xff\\xff\\x00\\xff\\xff\\xff\\xff\\xff\\xff\\xff\\xff\\xfe\\xff\\xff\\xff:/usr/local/bin/rosetta:OCFP";

pub fn setup_rosetta() -> Result<()> {
    let rosetta_path = find_in_path("rosetta").context("Failed to check existence of `rosetta`")?;
    let Some(_rosetta_path) = rosetta_path else {
        return Err(anyhow!("Failed to find `rosetta` in PATH"));
    };

    let mut file = File::options()
        .write(true)
        .open("/proc/sys/fs/binfmt_misc/register")
        .context("Failed to open binfmt_misc/register for writing")?;

    {
        let rule = ROSETTA_BINFMT_MISC_RULE;
        file.write_all(rule.as_bytes())
            .context("Failed to register `Rosetta` binfmt_misc rule")?;
    }

    Ok(())
}
