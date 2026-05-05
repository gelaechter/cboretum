# Cboretum

*A command line utility that allows you to "pack" a directory and it's contents into a [CBOR](https://cbor.io/) representation*

## What is this?

I made this to get around the file-amount limitation of [typst.app](https://typst.app/) since I often need many (but only small) files when documenting source code.

Cboretum turns an entire directory like this:
```
dir1
├── dir2
│   ├── file1.txt
│   └── file2.bin
├── file3.txt
└── file4.bin
```

into a CBOR file with the same structure:
```
dir1
├── dir2
│   ├── file1.txt
│   │   └── Text
│   └── file2.bin
│       └── Binary
├── file3.txt
│   └── Text
└── file4.bin
    └── Binary
```

So we can access these files with path completion in our Typst documents:
```typst
#let dir1 = cbor("dir1.cbor")

Here are the contents of file1.txt:
#raw(dir1.dir2.at("file1.txt"))
```

See [here](https://typst.app/project/riGN7vPFJCv710h23zD78A) for an example document
which displays all 1690 lucide icons:
```typst
// This cbor "archive" contains 3380 files
#let icons = cbor("icons.cbor")

#grid(
  columns: 20,
  gutter: 1em,
  ..icons.svg.values().map(svg => image(
      bytes(svg), // In the case of SVG you need to convert the text to
                  // bytes so Typst knows that it's an image.
      format: "svg")
    )
)
```

## Installation

You can install cboretum from [crates.io](https://crates.io/) using cargo:
```shell
cargo install cboretum
```

## Usage
```
Usage: arbor [OPTIONS] <PATH>

Arguments:
  <PATH>
          Path to the directory to convert

Options:
  -m, --max-size[=<MAX_SIZE>]
          Only include files up to a certain size
          
          You may specify a size using equals:
            --max-size      => defaults to 40KiB
            --max-size=1024 => limit of 1024 bytes
            --max-size=3MB  => limit of 3MB

  -t, --text-only
          Only include files that are valid UTF-8 text
          
          On occasion this may include non-text files if their contents happen to be parsable UTF-8

  -i, --include-file-name
          Changes leaf content to (filename, content) pairs
          
          This is useful if you intend to display the filename along the file contents

  -v, --verbose...
          Increase verbosity (repeat for more: -v info, -vv debug, -vvv trace)

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
