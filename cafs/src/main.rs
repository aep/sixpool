extern crate fuse;
extern crate time;
extern crate libc;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate sha2;
extern crate digest;
extern crate cdc;

use std::env;
use std::ffi::OsStr;

mod fs;
mod serializer;
mod index;
mod blockstore;
mod readchain;



fn main() {
    let i   = env::args_os().nth(1).unwrap();

    let mut bs = blockstore::new();
    let mut hi = index::from_host(i);
    hi.serialize(&mut bs);

    let j   = serde_json::to_string(&hi).unwrap();
    println!("{}", j);


    //for (hash,block) in bs.blocks {
    //    println!("{}", hash);
    //}

    let fs = fs::Fuse::new(&hi, &bs);

    let mountpoint  = env::args_os().nth(2).unwrap();
    let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("auto_unmount")];
    fuse::mount(fs, &mountpoint, &fuse_args).unwrap();

}
