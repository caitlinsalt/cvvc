use std::path::PathBuf;

use crate::repo::Repository;

/// Entry point for the `cv init` command.
pub fn cmd(pathname: &str) -> Result<(), anyhow::Error> {
    println!("Creating repository {pathname}");
    Repository::create(&PathBuf::from(pathname))?;
    Ok(())
}
