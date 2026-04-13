# zipbit11
A CLI tool for modifying bit 11 (the UTF-8 flag) in ZIP entry General Purpose Bit Flags.

![zipbit11](https://github.com/user-attachments/assets/e1a17bcb-de7d-4fa8-a61e-2f21b71f557e)

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/yuk7/zipbit11/ci.yml?style=flat-square)](https://github.com/yuk7/zipbit11/actions/workflows/ci.yml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=flat-square)](http://makeapullrequest.com)
![License](https://img.shields.io/github/license/yuk7/zipbit11.svg?style=flat-square)

[日本語](README_ja.md)

### [⬇Download](https://github.com/yuk7/zipbit11/releases/latest)

## Why
The ZIP format includes "bit 11 of the General Purpose Bit Flag" (the UTF-8 flag), which indicates that filenames are encoded in UTF-8. In practice, however, even ZIP files created with UTF-8 filenames may not have this flag set.

Depending on the OS or extraction tool, when the UTF-8 flag is not set, filenames may be interpreted using a different encoding. This can cause garbled filenames.

With this tool, you can edit the ZIP file directly to add or remove the UTF-8 flag. A small fix before sending a file or after receiving one can sometimes resolve filename corruption.

## Caution
This tool edits ZIP files directly.
Always back up important files before using it.

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

## Examples
### Mark ZIP contents as UTF-8
```bash
zipbit11 set archive.zip
```

### Remove the UTF-8 mark from ZIP contents
```bash
zipbit11 clear archive.zip
```

### Check the status of ZIP contents
```bash
zipbit11 detail archive.zip

# Output
File: archive.zip
Entries: 4
bit11: △ partial (2/4)

 No.   bit11   Filename
 ------------------------------------------------------------
 1     ✗ clear  English/
 2     ✗ clear  English/cat.txt
 3     ✓ set    日本語/
 4     ✓ set    日本語/猫.txt
```

## License
[MIT](LICENSE)
