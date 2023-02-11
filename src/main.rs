mod address;
mod btrfs;
mod btrfs_node;
mod dump;
mod mapped_file;
mod tree;
mod structures;
use clap::Parser;

/// access internal structures in an unmounted btrfs filesystem
///
/// Each available block device in the filesystem should be specified on the command line.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Params {
    #[clap(required = true)]
    paths: Vec<std::path::PathBuf>,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Params::parse();

    btrfs::dump(&args.paths)?;

    Ok(())
}
