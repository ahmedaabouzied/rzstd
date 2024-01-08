use std::env;
use std::fs::File;
use std::process;
use tokio::sync::broadcast;

use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;

use anyhow::Result;
use colored::Colorize;
use futures::future::join_all;

mod progress;
use progress::Progress;

#[tokio::main]
async fn main() -> Result<()> {
    // Collect file paths from command line arguments
    let args: Vec<String> = env::args().collect();

    // Check that we have at least one file path
    if args.len() < 2 {
        eprintln!("Usage: rzstd <regex> <file1> <file2> ...");
        process::exit(1);
    }

    let regex = &args[1];
    let files = &args[2..];

    // handles is a vector of futures that will be executed concurrently
    let mut handles = Vec::new();
    let mut progress_receivers = Vec::new();
    let mut total_receivers = Vec::new();
    for file_path in files {
        let (progress_sender, progress_receiver) = broadcast::channel(1);
        let (total_sender, total_receiver) = broadcast::channel(1);
        progress_receivers.push(progress_receiver);
        total_receivers.push(total_receiver);

        let regex = regex.clone(); // Clone regex for each task
        let file_path = file_path.clone(); // Clone file_path for each

        // Spawn a task to process for the file
        let handle = tokio::spawn(async move {
            match process_file(&file_path, &regex, progress_sender, total_sender).await {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Error processing file {}: {}", file_path, e);
                    process::exit(1);
                }
            }
        });
        // Add the task to the vector of tasks
        handles.push(handle);
    }

    // Spawn a task to print progress
    let progress = tokio::spawn(async move {
        let mut bytes_read = 0;
        let mut total = 0;

        loop {
            for total_receiver in &mut total_receivers {
                match total_receiver.try_recv() {
                    Ok(bytes) => total += bytes,
                    Err(_) => (),
                }
            }
            for progress_receiver in &mut progress_receivers {
                match progress_receiver.try_recv() {
                    Ok(bytes) => bytes_read += bytes,
                    Err(_) => (),
                }
            }
            if total == 0 {
                continue;
            }
            if bytes_read >= total {
                eprint!("Decompression 100% done \n");
                break;
            }

            let percent = (bytes_read as f64 / total as f64) * 100.0;
            eprint!("Decompression {:.2}% done \n", percent);
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    });

    // Join all the tasks and wait for them all to complete
    let _ = join_all(handles).await;
    let _ = progress.await;

    Ok(())
}

/// Processes a single file.
/// It will stream the file into a decoder and stream the
/// decoded data into a searcher. The searcher will then
/// perform a regext "grep" and print the results to stdout.
async fn process_file(
    file_path: &str,
    regex: &str,
    progress_sender: broadcast::Sender<usize>,
    total_sender: broadcast::Sender<usize>,
) -> Result<()> {
    let file = match File::open(file_path) {
        Ok(file) => file,
        Err(e) => {
            let e = anyhow::anyhow!("Error opening file {}: {}", file_path, e);
            return Err(e.into());
        }
    };

    if file.metadata()?.len() == 0 {
        // File is empty, nothing to do
        return Ok(());
    }
    total_sender.send(file.metadata()?.len() as usize).unwrap();

    if file.metadata()?.file_type().is_dir() {
        // File is a directory, nothing to do
        return Err(anyhow::anyhow!("{} is a directory", file_path));
    }

    if file.metadata()?.file_type().is_symlink() {
        // File is a symlink, nothing to do
        // we don't follow symlinks
        return Err(anyhow::anyhow!("{} is a symlink", file_path));
    }

    let p = Progress::new(file, progress_sender);

    // Read zstd encoded data from stdin and decode
    let decoder = match zstd::stream::read::Decoder::new(p) {
        Ok(decoder) => decoder,
        Err(e) => {
            let e = anyhow::anyhow!("Error creating decoder for file {}: {}", file_path, e);
            return Err(e.into());
        }
    };

    let matcher = match RegexMatcher::new(&regex) {
        Ok(matcher) => matcher,
        Err(e) => {
            let e = anyhow::anyhow!("Error compiling regex {}: {}", regex, e);
            return Err(e.into());
        }
    };

    match Searcher::new().search_reader(
        &matcher,
        decoder,
        UTF8(|_lnum, line| {
            // Color the matched string to red.
            let matched_str = match matcher.find(line.as_bytes()) {
                Ok(matched_str) => matched_str,
                Err(_) => return Ok(true), // Return true in the lambda function to continue searching
            };
            let matched_str = match matched_str {
                Some(matched_str) => matched_str,
                None => return Ok(true), // Return true in the lambda function to continue searching
            };
            let colored_line =
                line.replace(&line[matched_str], &line[matched_str].red().to_string());

            // Print the line to stdout
            // Here we use print!() instead of println!() because
            // each line already has a newline character at the end.
            print!("{}", colored_line);
            Ok(true) // Return true in the lambda function to continue searching
        }),
    ) {
        Ok(_) => (),
        Err(e) => {
            let e = anyhow::anyhow!("Error searching file {}: {}", file_path, e);
            return Err(e.into());
        }
    };

    Ok(())
}
