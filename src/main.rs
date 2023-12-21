use std::fs::File;
use std::env;
use std::process;

use grep_regex::RegexMatcher;
use grep_matcher::Matcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;

use anyhow::Result;
use futures::future::join_all;
use colored::Colorize;


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
    for file_path in files {
        let regex = regex.clone(); // Clone regex for each task
        let file_path = file_path.clone(); // Clone file_path for each
                                           
        // Spawn a task to process for the file
        let handle = tokio::spawn(async move {
            match process_file(&file_path, &regex).await {
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

    // Join all the tasks and wait for them all to complete
    let _ = join_all(handles).await;


    Ok(())
}

/// Processes a single file. 
/// It will stream the file into a decoder and stream the 
/// decoded data into a searcher. The searcher will then
/// perform a regext "grep" and print the results to stdout.
async fn process_file(file_path: &str, regex: &str) -> Result<()> {
    let file = match File::open(file_path){
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

    match Searcher::new().search_reader(&matcher, decoder, UTF8(|_lnum, line| {
        // Color the matched string to red.
        let matched_str = match matcher.find(line.as_bytes()) {
            Ok(matched_str) => matched_str,
            Err(_) => return Ok(true), // Return true in the lambda function to continue searching
        };
        let matched_str = match matched_str {
            Some(matched_str) => matched_str,
            None => return Ok(true), // Return true in the lambda function to continue searching
        };
        let colored_line = line.replace(&line[matched_str], &line[matched_str].red().to_string());

        // Print the line to stdout
        // Here we use print!() instead of println!() because
        // each line already has a newline character at the end.
        print!("{}", colored_line);
        Ok(true) // Return true in the lambda function to continue searching
    })){
        Ok(_) => (),
        Err(e) => {
            let e = anyhow::anyhow!("Error searching file {}: {}", file_path, e);
            return Err(e.into());
        }
    };

    Ok(())
}
