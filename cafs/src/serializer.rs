use std::fs::File;
use std::io::{Read, BufReader};
use cdc::{RollingHash64,Rabin64};
use sha2::{Sha512, Digest};
use index::*;
use blockstore::{Block, BlockStore, BlockShard};

fn predicate(x: u64) -> bool {
    const BITMASK: u64 = (1u64 << 14) - 1;
    x & BITMASK == BITMASK
}

struct IntermediateBlockRef {
    inode:   u64,
    blockno: usize,
    offset:  usize, //where the file starts inside the block
    size:    usize,
    foffset: usize, //where the block starts in the file
}

impl Index {

    pub fn serialize(&mut self, blockstore: &mut BlockStore) {

        // window_size: 1 << 6 == 64 bytes
        let separator_size_nb_bits = 6;
        let mut rabin  = Rabin64::new(separator_size_nb_bits);
        let mut hasher = Sha512::default();

        let mut curent_block_nr      = 0; //linear count only for intermediate
        let mut current_block_offset = 0; //current offset into the block (since the last file end)
        let mut current_file_offset  = 0; //where the file was when we ended the last block
        let mut current_file_len     = 0; //current length of the file being read (since the last block start)
        let mut current_block_len    = 0; //just the size of the block

        let mut intermediate : Vec<IntermediateBlockRef> = Vec::new();
        let mut blocks : Vec<String> = Vec::new();
        let mut blocklens : Vec<usize> = Vec::new();

        for inode in &self.inodes {
            if inode.k != 2 {
                continue;
            }
            let mut file = BufReader::new(File::open(&inode.host_path).unwrap()).bytes();

            while let Some(byte) = file.next() {
                current_file_len  += 1;
                current_block_len += 1;
                let b = byte.unwrap();
                hasher.input(&[b]);
                rabin.slide(&b);
                if (predicate)(rabin.hash) {
                    let hs = format!("{:x}", hasher.result());
                    blocks.push(hs);
                    blocklens.push(current_block_len);
                    hasher = Sha512::default();

                    intermediate.push(IntermediateBlockRef{
                        inode:   inode.i,
                        blockno: curent_block_nr,
                        offset:  current_block_offset,
                        size:    current_file_len,
                        foffset: current_file_offset,
                    });
                    current_file_offset += current_file_len;
                    current_file_len = 0;
                    current_block_offset = 0;
                    curent_block_nr += 1;
                    current_block_len = 0;
                }
            }

            intermediate.push(IntermediateBlockRef{
                inode:   inode.i,
                blockno: curent_block_nr,
                offset:  current_block_offset,
                size:    current_file_len,
                foffset: current_file_offset,
            });
            current_block_offset = current_file_len;
            current_file_len = 0;
            current_file_offset = 0;
        }
        let hs = format!("{:x}", hasher.result());
        blocks.push(hs);
        blocklens.push(current_block_len);

        let mut fblocks = Vec::new();
        for x in 0..blocks.len() {
            fblocks.push(Block{
                shards: Vec::new(),
                size:   blocklens[x],
            })
        }

        for int in intermediate {
            fblocks[int.blockno].shards.push(BlockShard{
                file:    self.inodes[int.inode as usize].host_path.clone(),
                offset:  int.foffset,
                size:    int.size,
            });

            match self.inodes[int.inode as usize].c {
                None => panic!("BUG: me fail borrowck"),
                Some(ref mut c) => c.push(ContentBlockEntry{
                    h: blocks[int.blockno].clone(),
                    o: int.offset as u64,
                    l: int.size as u64,
                }),
            }
        }

        for _ in 0..blocks.len() {
            blockstore.insert(blocks.remove(0), fblocks.remove(0));
        }
    }
}

