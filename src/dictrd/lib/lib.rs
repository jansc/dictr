use self::errors::DictError;
use log::info;
use rand::seq::SliceRandom;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
pub mod errors;
pub mod parser;

#[derive(Clone)]
pub struct IndexEntry {
    pub word: String,
    pub offset: u64,
    pub length: u64,
}

pub struct IndexReader {
    idx: Vec<IndexEntry>,
}

impl Default for IndexReader {
    fn default() -> Self {
        IndexReader::new()
    }
}

impl IndexReader {
    pub fn new() -> IndexReader {
        IndexReader { idx: Vec::new() }
    }

    fn decode_base64(&mut self, word: &str) -> Result<u64, DictError> {
        let mut index = 0u64;
        for (i, ch) in word.chars().rev().enumerate() {
            let base64 = match ch {
                '0'..='9' => (ch as u64) + 4,
                'A'..='Z' => (ch as u64) - 65,
                'a'..='z' => (ch as u64) - 71,
                '+' => 62,
                '/' => 63,
                _ => return Err(DictError::InvalidBase64),
            };
            index += base64 * 64u64.pow(i as u32);
        }
        Ok(index)
    }

    pub fn find_word(&mut self, word: &str) -> Result<(u64, u64), DictError> {
        let word = word.to_string();
        match self.idx.binary_search_by(|entry| entry.word.cmp(&word)) {
            Ok(idx) => {
                let entry = &self.idx[idx];
                //debug!("{}: {}", entry.offset, entry.length);
                Ok((entry.offset, entry.length))
            }
            Err(_e) => Err(DictError::NoMatch("552 no match")),
        }
    }

    pub fn find_words_by_prefix(&mut self, word: &str) -> Result<Vec<IndexEntry>, DictError> {
        let word = word.to_string();
        let mut res: Vec<IndexEntry> = Vec::new();
        for entry in self.idx.iter() {
            if entry.word.starts_with(word.as_str()) {
                res.push(entry.clone());
            }
        }
        Ok(res)
    }

    pub fn find_random(&mut self) -> Result<(String, u64, u64), DictError> {
        if let Some(res) = self.idx.choose(&mut rand::thread_rng()) {
            return Ok((res.word.clone(), res.offset, res.length));
        }
        Err(DictError::NoMatch("552 no match"))
    }

    pub fn parse_dict_index<B: BufRead>(&mut self, buf: B) {
        let mut line_number = 0;
        for l in buf.lines() {
            let line = l.unwrap();
            let entry = self.parse_line(&line);
            self.idx.push(entry);
            line_number += 1;
        }
        self.idx.sort_by(|e1, e2| e1.word.cmp(&e2.word));
        info!("Read {} lines from index", line_number);
    }

    fn parse_line(&mut self, line: &str) -> IndexEntry {
        let mut split = line.split('\t');
        let word = split.next().unwrap();
        let offset = split.next().unwrap();
        let offset = self.decode_base64(offset).unwrap();
        let length = split.next().unwrap();
        let length = self.decode_base64(length).unwrap();
        IndexEntry {
            word: word.to_owned(),
            offset,
            length,
        }
    }
}

pub struct DictReader<R: Read + Seek> {
    buf: BufReader<R>,
    len: u64,
}

impl<R: Read + Seek> DictReader<R> {
    pub fn new(mut buf: BufReader<R>) -> Result<DictReader<R>, std::io::Error> {
        let len = buf.seek(SeekFrom::End(0))?;
        Ok(DictReader { buf, len })
    }

    pub fn find(&mut self, offset: u64, len: u64) -> Result<String, DictError> {
        if offset >= self.len || offset + len > self.len {
            return Err(DictError::SyntaxError(
                "501 Syntax error, illegal parameters",
            ));
        }
        self.buf.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0; len as usize];
        self.buf.read_exact(&mut buffer)?;

        let result = String::from_utf8(buffer)?;
        //debug!("RESULT = {}", result);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::path::PathBuf;

    #[test]
    fn index_read() {
        let mut di = IndexReader::new();
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("db.expect.index");
        let file = File::open(path).unwrap();
        let file = BufReader::new(file);
        di.parse_dict_index(file);
    }

    #[test]
    fn dict_read() {
        let mut di = IndexReader::new();
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("db.expect.index");
        let file = File::open(path).unwrap();
        let file = BufReader::new(file);
        di.parse_dict_index(file);
        if let Ok((offset, length)) = di.find_word("headword4") {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("tests");
            path.push("db.expect.dict");
            let file = File::open(path).unwrap();
            let file = BufReader::new(file);
            let mut dr = DictReader::new(file).unwrap();
            dr.find(offset, length);
        }
    }
}
