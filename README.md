# Usage
```
Usage: omnischedule [OPTIONS] <PROJECT_FILE>

Arguments:
  <PROJECT_FILE>  The configuration YAML file for the project

Options:
  -a, --add-resource <ADD_RESOURCE>  Add an additional resource with the form <name>=<count>
  -d, --search-depth <SEARCH_DEPTH>  The maximum depth of the search tree to use. Deep trees can take a long time to solve [default: 5]
  -o, --output-csv <OUTPUT_CSV>      Output the solution to a CSV file
  -s, --start-date <START_DATE>      Calculate the dates of the solution using the given start date
  -h, --help                         Print help
```

# Installation
```
git clone https://github.com/r-bar/omnischedule
cd omnischedule
cargo install --path .
```
