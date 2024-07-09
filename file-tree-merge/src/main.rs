use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{io, thread};
use tap::Pipe;

use file_tree_merge::change_tree::ChangeTree;
use file_tree_merge::change_tree_merge::MergeStrategy;
use file_tree_merge::path_walker::PathWalker;
use file_tree_merge::tree_creator::{Item, TreeCreator};
use file_tree_merge::{apply_change, change_tree, change_tree_merge, TREE_DIR};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        short = 'a',
        help = "First mount point, where actual files that needs to be synced are located."
    )]
    path1_mnt: PathBuf,

    #[arg(
        short,
        long,
        short = 'x',
        help = "A directory where we keep a git repo to detect changes. Should persist between runs. MUST NOT BE INSIDE ANY OF THE <PATH1-MNT> or <PATH2-MNT> DIRECTORIES>"
    )]
    path1_repo: PathBuf,

    #[arg(
        short,
        long,
        short = 'b',
        help = "Second mount point, where actual files that needs to be synced are located."
    )]
    path2_mnt: PathBuf,

    #[arg(
        short,
        long,
        short = 'y',
        help = "A directory where we keep a git repo to detect changes. Should persist between runs. MUST NOT BE INSIDE ANY OF THE <PATH1-MNT> or <PATH2-MNT> DIRECTORIES>"
    )]
    path_repo: PathBuf,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "This simulates the sync. Will not actually create or change any of the files in <PATH1-MNT> or <PATH2-MNT>, will just print the operations that would have normally be applied to both ends"
    )]
    dry_run: bool,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "If specified it will calculate MD5 hash for files when comparing file in <PATH1-MNT> with the file in <PATH2-MNT> when applying Add and Modify operations. It will be considerably slower when activated"
    )]
    checksum: bool,

    #[arg(
        short,
        long,
        short = 'r',
        default_value_t = false,
        help = "If specified it will skip CRC check after file was transferred. Without this it compares the CRC of the file in <PATH1-MNT> before transfer with the CRC of the file in <PATH2-MNT> after transferred. This ensures the transfer was successful. Checking CRC is highly recommend if any of <PATH1-MNT> or <PATH1-MNT> are accessed over the network"
    )]
    no_crc: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.no_crc {
        println!("{}", "CRC is disabled for checking file after transfer. Make sure you want it like that, if not, remove --no-crc from args !".red().bold());
        thread::sleep(std::time::Duration::from_secs(10));
    }

    if args.checksum {
        println!(
            "{}",
            "Checksum mode enabled, it could be very slow!"
                .yellow()
                .bold()
        );
    }

    if args.dry_run {
        println!("{}", "Dry-run mode enabled, it will not touch any files on dst, will just print the changes!".yellow().bold());
    }

    println!("{}", "Build changes trees...".cyan());
    let (changes_tree1, errors1) =
        changes_tree(PathWalker::new(&args.path1_mnt), &args.path1_repo)?;
    let (changes_tree2, errors2) = changes_tree(PathWalker::new(&args.path2_mnt), &args.path_repo)?;

    println!("{}", "Merge changes trees...".cyan());
    change_tree_merge::merge(changes_tree1, changes_tree2, MergeStrategy::OneWay)?.pipe(|x| {
        if x.0 .0.is_empty() && x.1 .0.is_empty() {
            println!("{}", "No changes to apply".green());
            return Ok(());
        }
        if !args.dry_run {
            println!("{}", "Apply changes...".cyan());
        }
        println!("{}", "src -> dst...".cyan());
        let (changes_src, items_src) = x.0;
        let (_changes_dst, items_dst) = x.1;
        apply_change::apply(
            &changes_src,
            &items_src,
            &items_dst,
            &args.path1_mnt,
            &args.path2_mnt,
            args.dry_run,
            args.checksum,
            !args.no_crc,
        )
        // todo: dst -> src
    })?;

    if !errors1.is_empty() {
        println!("{}", "Errors reading from src:".red());
        for e in errors1 {
            println!("{}", e.to_string().red().bold());
        }
    }

    if !errors2.is_empty() {
        println!("{}", "Errors reading from dst:".red());
        for e in errors2 {
            println!("{}", e.to_string().red().bold());
        }
    }

    Ok(())
}

fn changes_tree(
    iter: PathWalker,
    repo: &Path,
) -> Result<((ChangeTree, BTreeMap<String, Item>), Vec<io::Error>)> {
    iter.pipe(TreeCreator::new)
        .pipe(|x| x.create(&repo.join(TREE_DIR)))?
        .pipe(|x| Ok((change_tree::build(x.0, repo)?, x.1)))
}
