use blockstore::{BlockStore,Block};
use fuse::*;
use index::{Index, Inode};
use libc::ENOENT;
use readchain::{ReadChain, ReadChainAble};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, BufReader, Result};
use time::Timespec;
use std::boxed::Box;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 };                 // 1 second

const CREATE_TIME: Timespec = Timespec { sec: 1381237736, nsec: 0 };    // 2013-10-08 08:56

const HELLO_TXT_CONTENT: &'static str = "Hello World!\n";

fn entry_to_file_attr(entry: &Inode) -> FileAttr{
    FileAttr {
        ino: entry.i + 1,
        size: entry.s,
        blocks: entry.s * 512,
        atime: CREATE_TIME,
        mtime: CREATE_TIME,
        ctime: CREATE_TIME,
        crtime: CREATE_TIME,
        kind: match entry.k {
            1 => FileType::Directory,
            _ => FileType::RegularFile,
        },
        perm: entry.a,
        nlink: match entry.d {
            Some(ref d) => d.len() + 1,
            _ => 1,
        } as u32,
        uid: 1000,
        gid: 1000,
        rdev: 0,
        flags: 0,
    }
}


pub struct Fuse<'a> {
    index:      &'a Index,
    blockstore: &'a BlockStore,
    openFiles:  HashMap<u64, Box<Read + 'a>>,
}

impl<'a> Fuse<'a> {
    pub fn new(index: &'a Index, blockstore: &'a BlockStore) -> Fuse<'a> {
        Fuse{
            index: index,
            blockstore: blockstore,
            openFiles: HashMap::new(),
        }
    }
}

impl<'a>  Filesystem for Fuse<'a> {
    fn lookup (&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {

        let mb = self.index.inodes.get((parent - 1) as usize)
            .and_then(|entry| entry.d.as_ref())
            .and_then(|d| d.get(&name.to_string_lossy().into_owned()))
            .and_then(|e| self.index.inodes.get(e.i as usize));

        match mb {
            None => reply.error(ENOENT),
            Some(entry) => {
                let fa = &entry_to_file_attr(entry);
                reply.entry(&TTL, fa, 0)
            }
        }
    }

    fn getattr (&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr {:?}", ino);

        match self.index.inodes.get((ino - 1) as usize) {
            None => reply.error(ENOENT),
            Some(entry) => {
                reply.attr(&TTL, &entry_to_file_attr(entry));
            }
        }
    }


    fn open(&mut self, _req: &Request, ino: u64, _flags: u32, reply: ReplyOpen) {
        println!("open {:?}", ino);
        match self.index.inodes.get((ino - 1) as usize) {
            None => {reply.error(ENOENT);},
            Some(entry) => {
                let mut fh = entry.i;
                while self.openFiles.contains_key(&fh) {
                    fh += 1;
                }
                self.openFiles.insert(fh, Box::new(ReadChain::new(InodeReader::new(entry, self.blockstore))));
                reply.opened(fh, 0);
            },
        };
    }
    fn release(&mut self,  _req: &Request, ino: u64, fh: u64,  _flags: u32, 
               _lock_owner: u64, _flush: bool, reply: ReplyEmpty) {
        println!("close {:?}", ino);
        self.openFiles.remove(&fh);
    }

    fn read (&mut self, _req: &Request, ino: u64, fh: u64, offset: u64, size: u32, reply: ReplyData) {
        println!("read {:?} {}", ino, offset);

        let file = self.openFiles.get_mut(&fh).unwrap();
        let mut buf = [0;512];
        let r = file.read(&mut buf).unwrap();
        println!("  > {}", r);
        reply.data(&buf[..r]);
    }

    fn readdir (&mut self, _req: &Request, ino: u64, _fh: u64, offset: u64, mut reply: ReplyDirectory) {
        println!("readdir {:?}", ino);
        if offset != 0 {
            reply.error(ENOENT);
            return;
        }
        match self.index.inodes.get((ino - 1) as usize) {
            None => reply.error(ENOENT),
            Some(entry) => {
                reply.add(1, 0, FileType::Directory, "."); //FIXME
                reply.add(1, 1, FileType::Directory, "..");

                let mut offset = 2;

                match entry.d {
                    None => reply.ok(),
                    Some(ref dir) => {
                        for (s,d) in dir {
                            reply.add(d.i, offset, match d.k {
                                1 => FileType::Directory,
                                _ => FileType::RegularFile,
                            }, s);
                            offset += 1;
                        }
                        reply.ok();
                    }
                };
            }
        }
    }
}

pub struct InodeReader<'a> {
    inode: &'a Inode,
    blockstore: &'a BlockStore,
}

impl<'a> InodeReader<'a> {
    pub fn new(inode: &'a Inode, blockstore: &'a BlockStore) -> InodeReader<'a> {
        InodeReader{
            inode: inode,
            blockstore: blockstore,
        }
    }
}

impl<'a> ReadChainAble<ReadChain<&'a Block, File>> for InodeReader<'a>{
    fn len(&self) -> usize {
        match self.inode.c {
            Some(ref c) => c.len(),
            _ => 0,
        }
    }
    fn at(&self, i: usize) -> (ReadChain<&'a Block, File>, usize) {
        let c = &self.inode.c.as_ref().unwrap()[i];
        let block = self.blockstore.get(&c.h).expect("block not found");
        let mut re = ReadChain::new(block);

        println!("opening block from {} len {} hash {}", c.o, c.l, c.h);

        re.seek(SeekFrom::Start(c.o as u64)).unwrap();
        (re, c.l as usize)
    }
}
