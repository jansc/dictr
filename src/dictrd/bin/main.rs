extern crate bufstream;
extern crate dictrdlib;
extern crate hostname;
extern crate os_info;
extern crate simple_logging;

use bufstream::BufStream;
use dictrdlib::parser::{Cmd, Command, Parser};
use dictrdlib::{DictReader, IndexEntry, IndexReader};
use log::LevelFilter;
use log::{debug, error, info};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fs::File;
use std::io::Write;
use std::io::{BufRead, BufReader, Read, Seek};
use std::net::SocketAddr;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::thread::spawn;

#[derive(Debug)]
pub enum DictdError {
    IoError(::std::io::Error),
    EncodingError(::std::string::FromUtf8Error),
    IllegalParameters,
}

impl Display for DictdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DictdError")
    }
}

impl std::error::Error for DictdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            DictdError::IllegalParameters => None,
            DictdError::EncodingError(ref e) => Some(e),
            DictdError::IoError(ref e) => Some(e),
        }
    }
}

impl From<::std::io::Error> for DictdError {
    fn from(err: ::std::io::Error) -> DictdError {
        DictdError::IoError(err)
    }
}

impl From<::std::string::FromUtf8Error> for DictdError {
    fn from(err: ::std::string::FromUtf8Error) -> DictdError {
        DictdError::EncodingError(err)
    }
}

pub struct Database<R: Read + Seek> {
    shortname: String,
    description: String,
    info: String,
    indexreader: Arc<RwLock<IndexReader>>,
    dictreader: Arc<RwLock<DictReader<R>>>,
}

pub struct DictdServer<R: Read + Seek> {
    strategies: Arc<RwLock<HashMap<&'static str, &'static str>>>,
    databases: Arc<RwLock<HashMap<String, Database<R>>>>,
}

impl<R: Read + Seek> Clone for DictdServer<R> {
    fn clone(&self) -> DictdServer<R> {
        let strategies = self.strategies.clone();
        let databases = self.databases.clone();
        DictdServer {
            strategies,
            databases,
        }
    }
}

impl<R: Read + Seek> Default for DictdServer<R> {
    fn default() -> Self {
        DictdServer::new()
    }
}

impl<R: Read + Seek> DictdServer<R> {
    pub fn new() -> DictdServer<R> {
        let strategies = Arc::new(RwLock::new(HashMap::new()));
        strategies
            .write()
            .unwrap()
            .insert("exact", "Match headwords exactly");
        strategies
            .write()
            .unwrap()
            .insert("prefix", "Match prefixes");
        let databases = Arc::new(RwLock::new(HashMap::new()));
        DictdServer {
            strategies,
            databases,
        }
    }

    // Adds a database to the server
    pub fn add_database(
        &mut self,
        shortname: String,
        description: String,
        info: String,
        indexreader: Arc<RwLock<IndexReader>>,
        dictreader: Arc<RwLock<DictReader<R>>>,
    ) {
        let database = Database {
            shortname: shortname.clone(),
            description,
            info,
            indexreader,
            dictreader,
        };
        self.databases.write().unwrap().insert(shortname, database);
    }

    // Handles a connection from the client
    // TODO: Should count commands and close connection after xx commands
    pub fn handle_connection(
        &mut self,
        stream: &mut BufStream<TcpStream>,
    ) -> Result<(), DictdError> {
        let mut parser = Parser::new();
        let info = os_info::get();
        stream.write_all(
            format!(
                "220 {:?} {} on {} {}\n",
                hostname::get()?,
                env!("CARGO_PKG_NAME"),
                info.os_type(),
                info.version()
            )
            .as_bytes(),
        )?;
        if let Err(err) = stream.flush() {
            return Err(DictdError::IoError(err));
        }
        loop {
            let mut reads = String::new();
            stream.read_line(&mut reads).unwrap(); //TODO: non-blocking read
            let query = reads.trim();
            if !query.is_empty() {
                info!(
                    "{}: Received query: {}",
                    stream.get_ref().peer_addr().unwrap(),
                    query
                );
                let result = parser.parse(query);
                let cmd = match result {
                    Ok(cmd) => cmd,
                    _ => {
                        stream.write_all(b"500 I/O error\n").unwrap();
                        stream.flush().unwrap();
                        continue;
                    }
                };
                match cmd.cmd {
                    Cmd::Define => {
                        if let Err(e) = self.command_define(&mut *stream, cmd) {
                            return Err(e);
                        }
                    }
                    Cmd::Help => {
                        if let Err(e) = self.command_help(&mut *stream) {
                            return Err(e);
                        }
                    }
                    Cmd::Match => {
                        if let Err(e) = self.command_match(&mut *stream, cmd) {
                            return Err(e);
                        }
                    }
                    Cmd::Show => {
                        if let Err(e) = self.command_show(&mut *stream, cmd) {
                            return Err(e);
                        }
                    }
                    Cmd::Status => {
                        if let Err(e) = self.command_status(&mut *stream, cmd) {
                            return Err(e);
                        }
                    }
                    Cmd::Quit => {
                        if let Err(e) = self.command_quit(&mut *stream, cmd) {
                            return Err(e);
                        }
                        break;
                    }
                    Cmd::Option => {
                        if let Err(e) = stream.write_all(b"502 OPTION not implemented\n") {
                            return Err(DictdError::IoError(e));
                        }
                    }
                    Cmd::Unknown => {
                        if cmd.params.len() == 1 && cmd.params[0] == "XRANDOM" {
                            if let Err(e) = self.command_random(&mut *stream, cmd) {
                                return Err(e);
                            }
                        } else if let Err(e) = stream.write_all(b"502 OPTION not implemented\n") {
                            return Err(DictdError::IoError(e));
                        }
                    }

                    _ => {
                        if let Err(e) = stream.write_all(b"500 Unknown Command\n") {
                            return Err(DictdError::IoError(e));
                        }
                    }
                }
                if let Err(e) = stream.flush() {
                    return Err(DictdError::IoError(e));
                }
                //debug!("DEBUG: reads len =>>>>> {}", reads.len());
            }
        }
        Ok(())
    }

    // Helper function
    fn database_exists(&self, database: &str) -> bool {
        if self.databases.read().unwrap().contains_key(database) {
            return true;
        }
        false
    }

    fn strategy_exists(&self, strategy: &str) -> bool {
        if self.strategies.read().unwrap().contains_key(strategy) {
            return true;
        }
        false
    }

    fn command_help(&self, stream: &mut BufStream<TcpStream>) -> Result<(), DictdError> {
        stream.write_all(b"113 help text follows\n")?;
        stream.write_all(b"DEFINE database word         -- look up word in database\n")?;
        stream.write_all(
            b"MATCH database strategy word -- match word in database using strategy\n",
        )?;
        stream.write_all(b"SHOW DB                      -- list all accessible databases\n")?;
        stream.write_all(b"SHOW DATABASES               -- list all accessible databases\n")?;
        stream
            .write_all(b"SHOW STRAT                   -- list available matching strategies\n")?;
        stream
            .write_all(b"SHOW STRATEGIES              -- list available matching strategies\n")?;
        stream.write_all(
            b"SHOW INFO database           -- provide information about the database\n",
        )?;
        stream.write_all(b"SHOW SERVER                  -- provide site-specific information\n")?;
        stream.write_all(b"OPTION MIME                  -- use MIME headers\n")?;
        //stream.write_all(b"CLIENT info                  -- identify client to server\n")?;
        //stream.write_all(b"AUTH user string             -- provide authentication information\n")?;
        stream.write_all(b"STATUS                       -- display timing information\n")?;
        stream.write_all(b"HELP                         -- display this help information\n")?;
        stream.write_all(b"XRANDOM                      -- return a random definition\n")?;
        stream.write_all(b"QUIT                         -- terminate connection\n.\n250 ok\n")?;
        Ok(())
    }

    fn command_define(
        &mut self,
        stream: &mut BufStream<TcpStream>,
        cmd: Command,
    ) -> Result<(), DictdError> {
        if cmd.params.len() < 3 {
            stream.write_all(b"501 Syntax error, illegal parameters\n")?;
            return Ok(());
        }
        let mut _match_all = false;
        let mut _match_one = false;
        let mut databases = Vec::<String>::new();
        let database = cmd.params[1].clone();
        match database.as_str() {
            "*" | "!" => {
                if database.as_str() == "*" {
                    _match_all = true;
                } else {
                    _match_one = true;
                }
                for d in self.databases.read().unwrap().keys() {
                    databases.push(d.clone());
                }
            }
            _ => {
                if !database.is_empty() && !self.database_exists(&database) {
                    stream.write_all(
                        b"550 Invalid database, use \"SHOW DB\" for list of databases\n",
                    )?;
                    return Ok(());
                }
                databases.push(database);
            }
        }
        let mut word = cmd.params[2].to_lowercase();
        word.retain(|c| c.is_alphanumeric() || c.is_whitespace());

        let database = &self.databases.read().unwrap()[&databases[0]];

        info!(
            "DEFINE from {}: DEFINE {} {}",
            stream.get_ref().peer_addr().unwrap(),
            cmd.params[1],
            word
        );
        // TODO: Loop over databases according to rules
        if let Ok((offset, length)) = database
            .indexreader
            .write()
            .unwrap()
            .find_word(word.as_str())
        {
            debug!("offset = {}, length = {}", offset, length);
            if let Ok(res) = database.dictreader.write().unwrap().find(offset, length) {
                stream.write_all(b"150 1 definition retrieved\n")?;
                stream.write_all(
                    format!(
                        "151 \"{}\" {} \"{}\"\n",
                        word, database.shortname, database.description
                    )
                    .as_bytes(),
                )?;
                stream.write_all(res.as_bytes())?;
                stream.write_all(b".\n")?;
                stream.write_all(b"250 ok\n")?;
            } else {
                stream.write_all(b"XXX NOT FOUND\n")?;
            }
        } else {
            stream.write_all(b"552 no match\n")?;
        }
        Ok(())
    }

    // MATCH database strategy word
    fn command_match(
        &mut self,
        stream: &mut BufStream<TcpStream>,
        cmd: Command,
    ) -> Result<(), DictdError> {
        if cmd.params.len() != 4 {
            stream.write_all(b"501 Syntax error, illegal parameters\n")?;
            return Ok(());
        }
        let strategy = &cmd.params[2];
        if !self.strategy_exists(&strategy) {
            stream.write_all(
                b"551 Invalid stragegy, use \"SHOW STRATS\" for a list of strategies\n",
            )?;
            return Ok(());
        }
        let word = &cmd.params[3];
        let mut _match_all = false;
        let mut _match_one = false;
        let mut databases = Vec::<String>::new();
        let database = cmd.params[1].clone();
        match database.as_str() {
            "*" | "!" => {
                if database.as_str() == "*" {
                    _match_all = true;
                } else {
                    _match_one = true;
                }
                for d in self.databases.read().unwrap().keys() {
                    databases.push(d.clone());
                }
            }
            _ => {
                if !database.is_empty() && !self.database_exists(&database) {
                    stream.write_all(
                        b"550 Invalid database, use \"SHOW DB\" for list of databases\n",
                    )?;
                    return Ok(());
                }
                databases.push(database);
            }
        }
        let word = word.to_lowercase();
        info!(
            "MATCH from {}: MATCH {:?} {} {}",
            stream.get_ref().peer_addr().unwrap(),
            cmd.params[1],
            strategy,
            word
        );

        let mut results: Vec<(String, IndexEntry)> = Vec::<(String, IndexEntry)>::new();

        for db in databases {
            match strategy.as_str() {
                "exact" => {
                    if let Ok((offset, length)) = &self.databases.read().unwrap()[&db]
                        .indexreader
                        .write()
                        .unwrap()
                        .find_word(word.as_str())
                    {
                        let entry = IndexEntry {
                            word: word.clone(),
                            offset: *offset,
                            length: *length,
                        };
                        results.push((db.clone(), entry));
                    }
                }
                "prefix" => {
                    if let Ok(res) = &self.databases.read().unwrap()[&db]
                        .indexreader
                        .write()
                        .unwrap()
                        .find_words_by_prefix(word.as_str())
                    {
                        for entry in res {
                            results.push((db.clone(), entry.clone()));
                        }
                    }
                }
                _ => (),
            }
        }

        // Collect results
        if !results.is_empty() {
            stream.write_all(
                format!("152 {} matche(s) found: list follows\n", results.len()).as_bytes(),
            )?;
            for (database, entry) in results {
                stream.write_all(format!("{} \"{}\"\n", database, entry.word).as_bytes())?;
            }
            stream.write_all(b".\n")?;
            stream.write_all(b"250 ok\n")?;
        } else {
            stream.write_all(b"552 no match\n")?;
        }
        Ok(())
    }

    fn command_random(
        &self,
        stream: &mut BufStream<TcpStream>,
        _cmd: Command,
    ) -> Result<(), DictdError> {
        if let Some(database) = self.databases.read().unwrap().get("jargon") {
            if let Ok((word, offset, length)) = database.indexreader.write().unwrap().find_random()
            {
                debug!("offset = {}, length = {}", offset, length);
                if let Ok(res) = database.dictreader.write().unwrap().find(offset, length) {
                    stream.write_all(b"150 1 definition retrieved\n")?;
                    stream.write_all(
                        format!(
                            "151 \"{}\" {} \"{}\"\n",
                            word, database.shortname, database.description
                        )
                        .as_bytes(),
                    )?;
                    stream.write_all(res.as_bytes())?;
                    stream.write_all(b".\n")?;
                    stream.write_all(b"250 ok\n")?;
                } else {
                    stream.write_all(b"552 no match\n")?;
                }
            } else {
                stream.write_all(b"552 no match\n")?;
            }
        }
        Ok(())
    }

    fn command_quit(
        &self,
        stream: &mut BufStream<TcpStream>,
        cmd: Command,
    ) -> Result<(), DictdError> {
        if cmd.params.len() != 1 {
            stream.write_all(b"501 Syntax error, illegal parameters\n")?;
            return Ok(());
        }
        stream.write_all(b"221 Closing connection. kthxb.\n")?;
        stream.flush()?;
        Ok(())
    }

    fn command_show(
        &self,
        stream: &mut BufStream<TcpStream>,
        cmd: Command,
    ) -> Result<(), DictdError> {
        if !cmd.params.len() == 2
            && !(cmd.params.len() == 3 && cmd.params[1].to_uppercase() == "INFO")
        {
            stream.write_all(b"501 Syntax error, illegal parameters\n")?;
            return Ok(());
        }
        match cmd.params[1].to_uppercase().as_str() {
            "DB" | "DATABASES" => {
                stream.write_all(
                    format!(
                        "110 {} database(s) present\n",
                        self.databases.read().unwrap().len()
                    )
                    .as_bytes(),
                )?;
                let databases = &*self.databases.read().unwrap();
                for (shortname, database) in databases {
                    stream.write_all(
                        format!("{} \"{}\"\n", shortname, database.description).as_bytes(),
                    )?;
                }
                stream.write_all(b".\n")?;
                stream.write_all(b"250 ok\n")?;
            }
            "STRAT" | "STRATEGIES" => {
                stream.write_all(
                    format!(
                        "111 {} strategies present\n",
                        self.strategies.read().unwrap().len()
                    )
                    .as_bytes(),
                )?;
                let strategies = &*self.strategies.read().unwrap();
                for (strat, descr) in strategies {
                    stream.write_all(format!("{} \"{}\"\n", strat, descr).as_bytes())?;
                }
                stream.write_all(b".\n")?;
                stream.write_all(b"250 ok\n")?;
            }
            "SERVER" => {
                stream.write_all(b"114 server information\n")?;
                stream.write_all(b"\n.\n")?;
            }
            "INFO" => {
                if cmd.params.len() != 3 {
                    stream.write_all(b"501 Syntax error, illegal parameters\n")?;
                } else {
                    let database = &cmd.params[2];
                    if !self.database_exists(database) {
                        stream.write_all(
                            b"550 Invalid database, use \"SHOW DB\" for list of databases\n",
                        )?;
                    } else {
                        let database = &self.databases.read().unwrap()[database];
                        stream.write_all(b"112 database information follows\n")?;
                        stream.write_all(database.description.as_bytes())?;
                        stream.write_all(b".\n")?;
                        stream.write_all(database.info.as_bytes())?;
                        stream.write_all(b".\n")?;
                        stream.write_all(b"250 ok\n")?;
                    }
                }
            }
            _ => {
                stream.write_all(b"501 Syntax error, illegal parameters\n")?;
            }
        }
        Ok(())
    }

    fn command_status(
        &self,
        stream: &mut BufStream<TcpStream>,
        cmd: Command,
    ) -> Result<(), DictdError> {
        if cmd.params.len() != 1 {
            return Ok(());
        }
        stream.write_all(b"210 status all good\n")?;
        Ok(())
    }
}

fn add_database(filename: String) -> (IndexReader, DictReader<File>, String, String) {
    let mut di = IndexReader::new();
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("dicts");
    path.push(format!("{}.index", filename));
    let file = File::open(path).unwrap();
    let file = BufReader::new(file);
    di.parse_dict_index(file);

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("dicts");
    path.push(format!("{}.dict", filename));
    let file = File::open(path).unwrap();
    let file = BufReader::new(file);
    let mut dr = DictReader::new(file).unwrap();

    let mut description = "Unknown".to_string();
    if let Ok((offset, length)) = di.find_word("00databaseshort") {
        if let Ok(res) = dr.find(offset, length) {
            let lines: Vec<&str> = res.split('\n').collect();
            if lines.len() >= 2 {
                description = lines[1].trim().to_string();
            }
        }
    }
    let mut info = "Unknown".to_string();
    if let Ok((offset, length)) = di.find_word("00databaseinfo") {
        if let Ok(res) = dr.find(offset, length) {
            let lines: Vec<&str> = res.split('\n').collect();
            if lines.len() >= 2 {
                info = lines[1].trim().to_string();
            }
        }
    }
    (di, dr, description, info)
}

fn main() {
    simple_logging::log_to_stderr(LevelFilter::Info);

    let port = 2628;
    let addr: SocketAddr = SocketAddr::from_str(format!("127.0.0.1:{}", port).as_str()).unwrap();
    let listener = TcpListener::bind(addr).unwrap_or_else(|e| {
        error!("Could not bind to port {}: {:?}", port, e);
        std::process::exit(1)
    });

    let mut dictd_server = DictdServer::<File>::new();
    let (di, dr, description, info) = add_database("jargon".to_string());
    dictd_server.add_database(
        "jargon".to_string(),
        description,
        info,
        Arc::new(RwLock::new(di)),
        Arc::new(RwLock::new(dr)),
    );
    let (di, dr, description, info) = add_database("devils".to_string());
    dictd_server.add_database(
        "devils".to_string(),
        description,
        info,
        Arc::new(RwLock::new(di)),
        Arc::new(RwLock::new(dr)),
    );
    for stream in listener.incoming() {
        match stream {
            Err(e) => error!("Could not listen to port: {:?}", e),
            Ok(stream) => {
                info!(
                    "New client connection from {} to {}",
                    stream.peer_addr().unwrap(),
                    stream.local_addr().unwrap()
                );
                let mut dictd_server = dictd_server.clone();
                spawn(move || {
                    let mut stream = BufStream::new(stream);
                    dictd_server.handle_connection(&mut stream).expect("Could not handle connection");
                });
            }
        }
    }
}
