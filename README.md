# Home Office Tracker
Simple tool to track which days you worked from home.

## Usage
```
Usage: hot.exe [OPTIONS] [COMMAND]

Commands:
  tui       Start the terminal user interface (default if no command is specified)
  add       Adds a date to the list of home office days
  remove    Removes a date from the list of home office days
  list      Lists all home office days
  data-dir  Prints the data directory
  export    Exports all home office days
  help      Print this message or the help of the given subcommand(s)

Options:
  -d, --data-dir <DATA_DIR>  The path to the data directory
  -h, --help                 Print help
```

The `add` and `remove` commands take an optional string argument.
The string can be a date in the format `%Y-%m-%d` or `%d.%m.%Y` or a range of dates, separated by `::`.