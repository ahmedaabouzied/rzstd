# RZSTD

A utility that does a grep over ZST compressed files in parallel.

## Problem definition

The problem this tries to solve is that `zstdgrep` processes files sequentially. With this small tool, each file gets processed in a separate concurrent task.

## Usage

```sh
rzst <regex> <file1> <file2> <file3>
```

## Example

```
rzst 'ID = 1' ./file1.zst ./file2.zst ./file3.zst
```

## Building

### Debug build: 

```
cargo build 
```

### Release build:
```
cargo build --Release
```
