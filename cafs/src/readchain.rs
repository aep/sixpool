use std::iter::Iterator;
use std::io::{Result, Read, Seek, SeekFrom, Error, ErrorKind};
use std::fs::File;
use std::cmp;

pub trait ReadChainAble<R>  where R: Read {

    /// number of Read traits available
    fn len(&self) -> usize;

    /// returns a Read trait at the position i, as well as a size
    /// the Read will only be read for `size` bytes, not until Eof.
    fn at(&self, i: usize) -> (R, usize);
}

/// ReadChain will take multiple Read and chain them together
/// so that the next Read is used when the previous ended.
pub struct ReadChain<T, R> where T: ReadChainAble<R>, R : Read {
    it: T,
    cur_index: usize,
    cur_size: usize,
    cur_file_consumed: usize,
    cur: Option<R>,
}

impl<T, R> ReadChain<T, R> where T: ReadChainAble<R>, R : Read {
    pub fn new(it: T) -> ReadChain<T, R>{
        ReadChain{
            it: it,
            cur_index: 0,
            cur_size: 0,
            cur_file_consumed: 0,
            cur: None,
        }
    }

    fn real_read(&mut self, buf: &mut [u8]) -> Result<usize> {
        println!("  > real_read {} {} {}", self.cur_index, buf.len(), self.cur_file_consumed);
        if let None = self.cur {
            if self.cur_index >= self.it.len() {
                return Ok(0);
            }
            let (f, s) = self.it.at(self.cur_index);
            self.cur = Some(f);
            self.cur_size = s;
            self.cur_file_consumed = 0;
        }

        if let Ok(rs) = self.cur.as_mut().unwrap().read(buf) {
            if rs > 0 {
                let rs = {
                    if self.cur_file_consumed + rs > self.cur_size {
                        self.cur_size - self.cur_file_consumed
                    } else {
                        rs
                    }
                };
                if rs > 0 {
                    self.cur_file_consumed += rs;
                    return Ok(rs);
                }
            }
        }

        self.cur = None;
        self.cur_index += 1;
        return self.real_read(buf);
    }
}

impl<T, R> Read for ReadChain<T, R> where T: ReadChainAble<R> , R: Read {
    // read is implemented to return less than requested on file border,
    // but most people just don't understand how berkley sockets work,
    // and some readers are broken.
    // So emulate the correct behaviour here
    // https://github.com/RustCrypto/hashes/issues/33
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let r = self.real_read(buf);
        match r {
            Err(e) => return Err(e),
            Ok(rs) => {
                if rs < buf.len() {
                    match self.real_read(&mut buf[rs..]) {
                        Err(e) => return Err(e),
                        Ok(rs2) => {return Ok(rs2+rs)}
                    }

                } else {
                    Ok(rs)
                }
            }
        }
    }
}

impl<T, R> Seek for ReadChain<T, R> where T: ReadChainAble<R> , R: Read + Seek  {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        println!("seeking {:?}", pos);
        if self.it.len() < 1 {
            return Ok(0);
        }
        match pos {
            SeekFrom::End(_) | SeekFrom::Current(_) => {
                return Err(Error::new(ErrorKind::NotFound, "not implemented"));
            },
            SeekFrom::Start(start)   => {
                self.cur_index = 0;
                self.cur = None;

                let mut seeked = 0;
                loop {
                    println!("  +> {}", self.cur_index);
                    if self.cur_index >= self.it.len() {
                        return Ok(seeked);
                    }
                    let (f, s) = self.it.at(self.cur_index);
                    self.cur = Some(f);
                    self.cur_size = s;
                    self.cur_file_consumed = 0;

                    let n = cmp::min(self.cur_size as u64, start - seeked);
                    match self.cur.as_mut().unwrap().seek(SeekFrom::Start(n)) {
                        Err(e) => return Err(e),
                        Ok(rs) => {
                            seeked += rs;
                            self.cur_file_consumed += rs as usize;
                            if rs >= self.cur_size as u64 {
                                self.cur = None;
                            }
                            if seeked >= start {
                                println!("  = seek ok");
                                return Ok(seeked);
                            }
                        }
                    }
                    self.cur_index += 1;
                }
            },
        };
    }
}

#[cfg(test)]
type TestVec = Vec<(&'static str,u64,usize)>;

#[cfg(test)]
impl ReadChainAble<File> for TestVec {
    fn len(&self) -> usize {
        (&self as &TestVec).len()
    }

    fn at(&self, i: usize) -> (File, usize) {
        let (f,o,l) = *self.get(i).unwrap();

        let mut f = File::open(f).unwrap();
        f.seek(SeekFrom::Start(o)).unwrap();
        (f,l)
    }
}

#[cfg(test)]
type TestVecVec = Vec<(TestVec,u64,usize)>;

#[cfg(test)]
impl ReadChainAble<ReadChain<TestVec, File>> for TestVecVec {
    fn len(&self) -> usize {
        (&self as &TestVecVec).len()
    }

    fn at(&self, i: usize) -> (ReadChain<TestVec, File>, usize) {
        let (ref f,o,l) = *self.get(i).unwrap();

        let mut f = ReadChain::new(f);

        f.seek(SeekFrom::Start(o)).unwrap();
        (f,l)
    }
}

#[test]
fn some_files() {
    let files : Vec<(&str,u64,usize)> = vec![
        ("tests/fixtures/a", 0, 4),
        ("tests/fixtures/b", 0, 4),
    ];

    let mut content = String::new();
    ReadChain::new(files).read_to_string(&mut content);
    assert_eq!(content, "yayacool");
}

#[test]
fn offset() {
    let files : Vec<(&str,u64,usize)> = vec![
        ("tests/fixtures/a", 1, 4),
        ("tests/fixtures/b", 4, 10),
    ];

    let mut content = String::new();
    ReadChain::new(files).read_to_string(&mut content);
    assert_eq!(content, "aya stuff");
}

