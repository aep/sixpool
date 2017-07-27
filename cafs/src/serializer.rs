use std::fs::File;
use std::io::{Read, BufReader};
use rollsum;

use sha2::{Sha512, Digest};
use index::*;
use blockstore::{Block, BlockStore, BlockShard};
use pbr::ProgressBar;
use std::ffi::OsString;
use std::io::Stdout;


struct IntermediateBlockRef {
    inode:       u64,
    file_start:  usize, //where the file was when the block started
    file_end:    usize, //where the file completed inside the block
    block_start: usize, //where the block was when the file started
}


fn print_progress_bar(bar: &mut ProgressBar<Stdout>, path: &OsString){
    let mut s = path.to_str().unwrap();
    if s.len() > 40 {
        bar.message(&format!("..{:38} ", &s[s.len()-38..]));
    } else {
        bar.message(&format!("{:40} ", &s));
    }
}

impl Index {
    fn emit_block(&mut self, blockstore: &mut BlockStore, len: usize, hash: String, inodes: &Vec<IntermediateBlockRef>) {

        let mut block_shards = Vec::new();

        for ibr in inodes {
            //println!("   inode {} at offset {} is {} into the block with size {}",
            //         ibr.inode, ibr.file_start, ibr.block_start, ibr.file_end - ibr.file_start);
            block_shards.push(BlockShard{
                file:    self.inodes[ibr.inode as usize].host_path.clone(),
                offset:  ibr.file_start,
                size:    ibr.file_end - ibr.file_start,
            });

            if let None = self.inodes[ibr.inode as usize].c {
                self.inodes[ibr.inode  as usize].c = Some(Vec::new());
            }
            self.inodes[ibr.inode as usize].c.as_mut().unwrap().push(ContentBlockEntry{
                h: hash.clone(),
                o: ibr.block_start as u64,
                l: (ibr.file_end - ibr.file_start) as u64,
            });
        }

        blockstore.insert(hash, Block{
            shards: block_shards,
            size: len,
        });
    }

    pub fn serialize(&mut self, blockstore: &mut BlockStore) {
        let mut bar = ProgressBar::new(self.inodes.len() as u64);
        bar.show_speed = false;
        bar.show_time_left = false;

        let mut chunker = rollsum::Bup::new_with_chunk_bits(13);
        let mut hasher  = Sha512::default();

        let mut current_block_len = 0;
        let mut current_files_in_block = Vec::new();
        let mut current_file_pos = 0;

        let inodes = self.inodes.to_vec();
        for inode in inodes {
            bar.inc();
            if inode.k != 2 {
                continue;
            }
            print_progress_bar(&mut bar, &inode.host_path);


            let mut file = BufReader::new(File::open(&inode.host_path).unwrap());
            current_files_in_block.push(IntermediateBlockRef{
                inode: inode.i,
                file_start: 0,
                file_end:   0,
                block_start: current_block_len,
            });

            let mut buf = [0;1024];
            loop {
                let rs = file.read(&mut buf).unwrap();
                if rs < 1 {
                    break;
                }

                if let Some(count) = chunker.find_chunk_edge(&buf[..rs]) {
                    current_block_len += count;
                    current_file_pos  += count;

                    current_files_in_block.last_mut().as_mut().unwrap().file_end = current_file_pos;

                    hasher.input(&buf[..count]);
                    let hash = format!("{:x}", hasher.result());
                    hasher  = Sha512::default();

                    self.emit_block(blockstore, current_block_len, hash, &current_files_in_block);
                    current_files_in_block.clear();
                    current_files_in_block.push(IntermediateBlockRef{
                        inode: inode.i,
                        file_start: current_file_pos,
                        file_end:   0,
                        block_start: 0,
                    });

                    hasher.input(&buf[count..rs]);
                    current_block_len  = rs - count;
                    current_file_pos  += rs - count;
                } else {
                    hasher.input(&buf[..rs]);
                    current_block_len += rs;
                    current_file_pos  += rs;
                }
            }
            current_files_in_block.last_mut().as_mut().unwrap().file_end = current_file_pos;
            current_file_pos = 0;
        }
        let hash = format!("{:x}", hasher.result());
        self.emit_block(blockstore, current_block_len, hash, &current_files_in_block);

        let total_block_size = blockstore.blocks.iter().fold(0, |acc, (_,b)| acc + b.size);
        let total_inode_size = self.inodes.iter().fold(0, |acc, i| acc + i.s);
        bar.finish_print("");


        let pc = (total_block_size as f32 / total_inode_size as f32) * 100.0;
        println!("done serializing {} inodes to {} blocks with total size of {} bytes ({:.0}% of inodes size)",
                 self.inodes.len(), blockstore.blocks.len(), total_block_size, pc);

    }
}
