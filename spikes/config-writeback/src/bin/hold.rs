//! Holds a file open the way an editor or another tool might, until its stdin closes.
//!
//! Usage: `hold <path> <share>`, where `<share>` is the Windows share mode as a number
//! (1 read, 2 write, 4 delete, summed). Prints `ready` once the file is open.

use std::{
    env,
    fs::OpenOptions,
    io::{self, Read, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let [path, share] = args.as_slice() else {
        eprintln!("usage: hold <path> <share>");
        return ExitCode::FAILURE;
    };
    let Ok(share) = share.parse::<u32>() else {
        eprintln!("share must be a number");
        return ExitCode::FAILURE;
    };
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, share);
    #[cfg(not(windows))]
    let _ = share;
    let _file = match options.open(path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("cannot open {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("ready");
    let _ = io::stdout().flush();
    let _ = io::stdin().read_to_end(&mut Vec::new());
    ExitCode::SUCCESS
}
