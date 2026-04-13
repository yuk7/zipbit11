mod zip_bit;

use std::process;

#[derive(Debug, PartialEq)]
enum ParsedArgs<'a> {
    Help,
    Run {
        mode: zip_bit::Mode,
        file_path: &'a str,
        selection: Option<&'a str>,
    },
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let parsed = match parse_args(&args) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!();
            print_help();
            process::exit(1);
        }
    };

    match parsed {
        ParsedArgs::Help => {
            print_help();
        }
        ParsedArgs::Run {
            mode,
            file_path,
            selection,
        } => match zip_bit::process(file_path, mode, selection) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                process::exit(1);
            }
        },
    }
}

fn parse_args(args: &[String]) -> Result<ParsedArgs<'_>, String> {
    match args.len() {
        1 => Ok(ParsedArgs::Help),
        2 => {
            let command = args[1].as_str();
            if is_help(command) {
                return Ok(ParsedArgs::Help);
            }
            if parse_subcommand(command).is_some() {
                return Err(format!("'{}' requires <file.zip>", command));
            }
            Err(format!("unknown subcommand: '{}'", command))
        }
        3 => {
            let command = args[1].as_str();
            let file_path = args[2].as_str();

            if is_help(command) {
                return Err("help does not take a file path".to_string());
            }

            let mode = parse_subcommand(command)
                .ok_or_else(|| format!("unknown subcommand: '{}'", command))?;
            Ok(ParsedArgs::Run {
                mode,
                file_path,
                selection: None,
            })
        }
        4 => {
            let command = args[1].as_str();
            let file_path = args[2].as_str();
            let selection = args[3].as_str();

            if is_help(command) {
                return Err("help does not take a file path".to_string());
            }

            let mode = parse_subcommand(command)
                .ok_or_else(|| format!("unknown subcommand: '{}'", command))?;

            match mode {
                zip_bit::Mode::Detail
                | zip_bit::Mode::Set
                | zip_bit::Mode::Clear
                | zip_bit::Mode::Toggle => {
                    Ok(ParsedArgs::Run {
                        mode,
                        file_path,
                        selection: Some(selection),
                    })
                }
                zip_bit::Mode::Status => {
                    Err(format!("'{}' does not take an entry selector", command))
                }
            }
        }
        _ => Err("too many arguments".to_string()),
    }
}

fn parse_subcommand(command: &str) -> Option<zip_bit::Mode> {
    match command {
        "status" => Some(zip_bit::Mode::Status),
        "detail" => Some(zip_bit::Mode::Detail),
        "set" => Some(zip_bit::Mode::Set),
        "clear" => Some(zip_bit::Mode::Clear),
        "toggle" => Some(zip_bit::Mode::Toggle),
        _ => None,
    }
}

fn is_help(command: &str) -> bool {
    command == "help" || command == "--help" || command == "-h"
}

fn print_help() {
    println!(
        "zipbit11 v{} — Manipulate bit 11 (UTF-8 flag) in ZIP file entries",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("USAGE:");
    println!("  zipbit11 <command> <file.zip> [entries]");
    println!("  zipbit11 help");
    println!();
    println!("COMMANDS:");
    println!("  status   Show entry count and overall bit 11 summary");
    println!("  detail   Show the summary and bit 11 status for all entries, or only [entries]");
    println!("  set      Set bit 11 (all entries, or only [entries])");
    println!("  clear    Clear bit 11 (all entries, or only [entries])");
    println!("  toggle   Toggle bit 11 (all entries, or only [entries])");
    println!("  help     Show this help");
    println!();
    println!("DESCRIPTION:");
    println!("  Sets or clears bit 11 (0x0800) of the General Purpose Bit Flag in each ZIP entry.");
    println!("  When bit 11 is set, the filename and comment are encoded in UTF-8.");
    println!("  For detail/set/clear/toggle, [entries] selects detail row numbers like: 1,3,5-8");
    println!();
    println!("  Warning: the file is modified in-place. Back up important files first.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_help_with_no_args() {
        let args = make_args(&["zipbit11"]);
        assert_eq!(parse_args(&args), Ok(ParsedArgs::Help));
    }

    #[test]
    fn parse_help_subcommand() {
        let args = make_args(&["zipbit11", "help"]);
        assert_eq!(parse_args(&args), Ok(ParsedArgs::Help));
    }

    #[test]
    fn parse_status_subcommand() {
        let args = make_args(&["zipbit11", "status", "sample.zip"]);
        assert_eq!(
            parse_args(&args),
            Ok(ParsedArgs::Run {
                mode: zip_bit::Mode::Status,
                file_path: "sample.zip",
                selection: None,
            })
        );
    }

    #[test]
    fn parse_detail_subcommand() {
        let args = make_args(&["zipbit11", "detail", "sample.zip"]);
        assert_eq!(
            parse_args(&args),
            Ok(ParsedArgs::Run {
                mode: zip_bit::Mode::Detail,
                file_path: "sample.zip",
                selection: None,
            })
        );
    }

    #[test]
    fn parse_set_with_entry_selector() {
        let args = make_args(&["zipbit11", "set", "sample.zip", "1,3-5"]);
        assert_eq!(
            parse_args(&args),
            Ok(ParsedArgs::Run {
                mode: zip_bit::Mode::Set,
                file_path: "sample.zip",
                selection: Some("1,3-5"),
            })
        );
    }

    #[test]
    fn reject_missing_file_path() {
        let args = make_args(&["zipbit11", "set"]);
        assert_eq!(
            parse_args(&args),
            Err("'set' requires <file.zip>".to_string())
        );
    }

    #[test]
    fn reject_unknown_subcommand() {
        let args = make_args(&["zipbit11", "enable", "sample.zip"]);
        assert_eq!(
            parse_args(&args),
            Err("unknown subcommand: 'enable'".to_string())
        );
    }

    #[test]
    fn parse_detail_with_entry_selector() {
        let args = make_args(&["zipbit11", "detail", "sample.zip", "1"]);
        assert_eq!(
            parse_args(&args),
            Ok(ParsedArgs::Run {
                mode: zip_bit::Mode::Detail,
                file_path: "sample.zip",
                selection: Some("1"),
            })
        );
    }

    #[test]
    fn reject_selector_for_status() {
        let args = make_args(&["zipbit11", "status", "sample.zip", "1"]);
        assert_eq!(
            parse_args(&args),
            Err("'status' does not take an entry selector".to_string())
        );
    }
}
