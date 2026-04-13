# zipbit11
A CLI tool for modifying bit 11 (the UTF-8 flag) in ZIP entry General Purpose Bit Flags.

## Usage
```bash
zipbit11 <command> <file.zip> [entries]
zipbit11 help
```

### Commands
- `status`: Show the entry count and overall bit 11 summary
- `detail`: Show the summary and bit 11 status for all entries, or only `[entries]`
- `set`: Set bit 11 for all entries, or only `[entries]`
- `clear`: Clear bit 11 for all entries, or only `[entries]`
- `toggle`: Toggle bit 11 for all entries, or only `[entries]`
- `help`: Show help

### Entry selector
- Use `detail` row numbers with comma-separated values and inclusive ranges, for example `1,3,5-8`

## Example
```bash
zipbit11 status archive.zip
zipbit11 detail archive.zip
zipbit11 detail archive.zip 1,3,5-8
zipbit11 set archive.zip
zipbit11 clear archive.zip 2,4,6-9
```

## License
[MIT](LICENSE)
