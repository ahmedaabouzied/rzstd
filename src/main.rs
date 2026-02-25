use std::fs::File;
use std::path::PathBuf;
use std::process;

use grep_regex::RegexMatcher;
use grep_matcher::Matcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;

use anyhow::Result;
use clap::Parser;
use futures::future::join_all;
use colored::Colorize;

/// Grep through zstd-compressed files in parallel.
///
/// rzstd decompresses each file concurrently and searches for lines
/// matching the given regex pattern, like `zstdgrep` but parallel.
#[derive(Parser)]
#[command(version, name = "rzstd")]
#[command(after_help = "Examples:\n  rzstd 'ERROR' server.log.zst\n  rzstd 'ID = 1' file1.zst file2.zst file3.zst")]
struct Cli {
    /// Regex pattern to search for
    pattern: String,

    /// One or more zstd-compressed files to search
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let regex = &cli.pattern;
    let files = &cli.files;

    // handles is a vector of futures that will be executed concurrently
    let mut handles = Vec::new();
    for file_path in files {
        let regex = regex.clone(); // Clone regex for each task
        let file_path = file_path.clone(); // Clone file_path for each

        // Spawn a task to process the file
        let handle = tokio::spawn(async move {
            let path_str = file_path.to_string_lossy().to_string();
            let lines = process_file(&path_str, &regex).await?;
            Ok::<_, anyhow::Error>((path_str, lines))
        });
        handles.push(handle);
    }

    let show_filename = files.len() > 1;

    // Join all tasks, then print results sequentially to avoid interleaving
    let results = join_all(handles).await;
    for result in results {
        match result {
            Ok(Ok((path, lines))) => {
                for line in lines {
                    if show_filename {
                        print!("{}:{}", path.magenta(), line);
                    } else {
                        print!("{}", line);
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
            Err(e) => {
                eprintln!("Task failed: {}", e);
                process::exit(1);
            }
        }
    }

    Ok(())
}

/// Processes a single file.
/// It will stream the file into a decoder and stream the
/// decoded data into a searcher. The searcher will then
/// perform a regex grep and return the matching lines.
async fn process_file(file_path: &str, regex: &str) -> Result<Vec<String>> {
    let file = match File::open(file_path){
        Ok(file) => file,
        Err(e) => {
            let e = anyhow::anyhow!("Error opening file {}: {}", file_path, e);
            return Err(e.into());
        }
    
    };

    if file.metadata()?.len() == 0 {
        // File is empty, nothing to do
        return Ok(Vec::new());
    }

    if file.metadata()?.file_type().is_dir() {
        // File is a directory, nothing to do
        return Err(anyhow::anyhow!("{} is a directory", file_path));
    }

    if file.metadata()?.file_type().is_symlink() {
        // File is a symlink, nothing to do
        // we don't follow symlinks
        return Err(anyhow::anyhow!("{} is a symlink", file_path));
    }

    // Read zstd encoded data from stdin and decode
    let decoder = match zstd::stream::read::Decoder::new(file){
        Ok(decoder) => decoder,
        Err(e) => {
            let e = anyhow::anyhow!("Error creating decoder for file {}: {}", file_path, e);
            return Err(e.into());
        }
    };

    let matcher = match RegexMatcher::new(&regex){
        Ok(matcher) => matcher,
        Err(e) => {
            let e = anyhow::anyhow!("Error compiling regex {}: {}", regex, e);
            return Err(e.into());
        }
    };

    let mut lines = Vec::new();

    match Searcher::new().search_reader(&matcher, decoder, UTF8(|_lnum, line| {
        // Color the matched string to red.
        let matched_str = match matcher.find(line.as_bytes()) {
            Ok(matched_str) => matched_str,
            Err(_) => return Ok(true),
        };
        let matched_str = match matched_str {
            Some(matched_str) => matched_str,
            None => return Ok(true),
        };
        let colored_line = line.replace(&line[matched_str], &line[matched_str].red().to_string());
        lines.push(colored_line);
        Ok(true)
    })){
        Ok(_) => (),
        Err(e) => {
            let e = anyhow::anyhow!("Error searching file {}: {}", file_path, e);
            return Err(e.into());
        }
    };

    Ok(lines)
}
