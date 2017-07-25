use std::fs::File;
use std::io::{Read, BufReader};
use rollsum;

use sha2::{Sha512, Digest};
use index::*;
use blockstore::{Block, BlockStore, BlockShard};

struct IntermediateBlockRef {
    inode:       u64,
    file_start:  usize, //where the file was when the block started
    file_end:    usize, //where the file completed inside the block
    block_start: usize, //where the block was when the file started
}


impl Index {
    fn emit_block(&mut self, blockstore: &mut BlockStore, len: usize, hash: String, inodes: &Vec<IntermediateBlockRef>) {
        println!("block {}, {} from {} files", hash, len, inodes.len());

        let mut block_shards = Vec::new();

        for ibr in inodes {
            println!("   inode {} at offset {} is {} into the block with size {}",
                     ibr.inode, ibr.file_start, ibr.block_start, ibr.file_end - ibr.file_start);
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

        let mut chunker = rollsum::Bup::new_with_chunk_bits(15);
        let mut hasher  = Sha512::default();

        let mut current_block_len = 0;
        let mut current_files_in_block = Vec::new();
        let mut current_file_pos = 0;

        let inodes = self.inodes.to_vec();
        for inode in inodes {
            if inode.k != 2 {
                continue;
            }

            let mut file = BufReader::new(File::open(&inode.host_path).unwrap());
            println!("reading {} {:?}", inode.i, inode.host_path);
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
    }



    /*

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
            let mut file = BufReader::new(File::open(&inode.host_path).unwrap());
            println!("reading {:?}", inode.host_path);


            let mut buf = [0;512];
            loop {
                let rs = file.read(&mut buf).unwrap();
                if rs < 1 {
                    break;
                }
                let bbuf = &buf[..rs];

                current_file_len  += rs;
                current_block_len += rs;

                hasher.input(bbuf);

                rabin.slide(&bbuf[0]); //TODO

                if (predicate)(rabin.hash) {
                    let hs = format!("{:x}", hasher.result());
                    blocks.push(hs);
                    println!(" > {}", current_block_len);
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

            println!(" > {}", current_block_len);
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
*/
}

