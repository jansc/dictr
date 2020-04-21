#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Cmd {
    Unknown, // 3.2
    Define,  // 3.2
    Match,   // 3.3
    Show,    // 3.5
    //  ShowDatabase, // 3.5.1
    //  ShowStrategies, // 3.5.2
    //  ShowInfo, // 3.5.3
    //  ShowServer, // 3.5.3
    Client,   // 3.6
    Status,   // 3.7
    Help,     // 3.8
    Quit,     // 3.9
    Option,   // 3.10
    Auth,     // 3.11
    SaslAuth, // 3.12
    SaslResp, // 3.12
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SubCmd {
    Unknown,
    Database,
    Strategies,
    Info,
    Server,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Command {
    pub cmd: Cmd,
    pub subcmd: SubCmd,
    pub strategy: String,
    pub database: String,
    pub params: Vec<String>,
}

pub struct CommandDesc {
    pub cmd_str: String,
    pub cmd: Cmd,
    pub min_params: i8,
}

pub struct Parser {
    pub result: Command,
}

impl Default for Parser {
    fn default() -> Self {
        Parser::new()
    }
}

impl Parser {
    pub fn new() -> Parser {
        let command = Command {
            cmd: Cmd::Unknown,
            subcmd: SubCmd::Unknown,
            strategy: String::new(),
            database: String::new(),
            params: Vec::<String>::new(),
        };
        Parser { result: command }
    }

    pub fn parse(&mut self, string: &str) -> Result<Command, std::io::Error> {
        let iter = string.chars();
        let mut arg = Vec::<char>::with_capacity(20);

        // True if arg parsed and whitespace found
        let mut skip_whitespace = false;
        let mut in_arg = false; // True if in an argument
        let mut in_dblquote = false;
        let mut args = Vec::<String>::new();
        let mut quote = false;
        // TODO: Implement single quotes
        for ch in iter {
            if quote {
                if in_dblquote && ch == '\"' {
                    arg.push(ch);
                }
                quote = false;
                continue;
            }
            if ch == '\\' {
                quote = true;
                continue;
            }
            if ch == '"' {
                if in_dblquote {
                    args.push(arg.clone().into_iter().collect::<String>());
                    arg.clear();
                    in_arg = false;
                    in_dblquote = false;
                    skip_whitespace = true;
                    arg.push(ch);
                } else {
                    in_dblquote = true;
                }
            }
            if ch.is_whitespace() {
                if in_dblquote {
                    arg.push(ch);
                } else {
                    if skip_whitespace {
                        continue;
                    }
                    args.push(arg.clone().into_iter().collect::<String>());
                    arg.clear();
                    skip_whitespace = true;
                }
            }
            if ch.is_alphanumeric() || ch.is_ascii_punctuation() && ch != '\"' {
                in_arg = true;
                if skip_whitespace {
                    skip_whitespace = false;
                }
                arg.push(ch);
            }
        }
        if in_arg {
            args.push(arg.into_iter().collect::<String>());
        }

        //debug!("Found {} args: {:?}", args.len(), args);
        let argc = args.len();
        if argc == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "501 Syntax error, illegal parameters",
            ));
        }
        let command = String::from_utf8_lossy(args[0].as_bytes());
        let cmd = match command.to_uppercase().as_str() {
            "DEFINE" => Cmd::Define,
            "MATCH" => Cmd::Match,
            "SHOW" => Cmd::Show,
            "CLIENT" => Cmd::Client,
            "STATUS" => Cmd::Status,
            "HELP" => Cmd::Help,
            "QUIT" => Cmd::Quit,
            "OPTION" => Cmd::Option,
            "AUTH" => Cmd::Auth,
            "SASLAUTH" => Cmd::SaslAuth,
            "SASLRESP" => Cmd::SaslResp,
            _ => Cmd::Unknown,
        };
        //println!("COMMAND={}, arg[1] = {}, cmd={:?}", command, args[1], cmd);
        Ok(Command {
            cmd,
            subcmd: SubCmd::Unknown,
            strategy: String::new(),
            database: String::new(),
            params: args,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_show() {
        let mut parser = Parser::new();
        let result = parser.parse("SHOW    DATABASE \"foo b\\\"ar\"").unwrap();
        println!("{:?}", result);
        assert_eq!(result.cmd, Cmd::Show);
    }

    #[test]
    fn parser_match() {
        let mut parser = Parser::new();
        let result = parser.parse("MATCH foldoc regex \"s.si\"").unwrap();
        println!("{:?}", result);
        assert_eq!(result.cmd, Cmd::Match);
    }

    #[test]
    fn parser_match_quotes() {
        let mut parser = Parser::new();
        let result = parser.parse("match jargon exact \"ack\"").unwrap();
        println!("{:?}", result);
        assert_eq!(result.cmd, Cmd::Match);
        assert_eq!(result.params[1], "jargon");
        assert_eq!(result.params[2], "exact");
        assert_eq!(result.params[3], "ack");
    }

    #[test]
    fn parser_define() {
        let mut parser = Parser::new();
        let result = parser.parse("DEFINE * shortcake").unwrap();
        println!("{:?}", result);
        assert_eq!(result.cmd, Cmd::Define);
    }
}
