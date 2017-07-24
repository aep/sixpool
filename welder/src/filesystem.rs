use std::fs::{File, create_dir_all};
use std::io::{BufRead, BufReader};
use nix::mount::{mount, MS_BIND, MS_REC, MS_SLAVE, umount, MsFlags};
use ::errors;
use nix::unistd::{chdir, chroot};
use nix;

pub struct Mount {
    pub source: String,
    pub target: String,
    pub fstype: String,
    pub flags: String,
}

pub struct Mounts {
    f: BufReader<File>,
}

pub fn mounts() -> errors::Result<Mounts> {
    let f = File::open("/proc/mounts")?;
    return Ok(Mounts {
        f: BufReader::new(f)
    })
}

impl Iterator for Mounts {
    type Item = Mount;
    fn next(&mut self) -> Option<Mount> {
        let mut line = String::new();
        match self.f.read_line(&mut line) {
            Err(_) => None,
            Ok(_) => {
                let vec: Vec<&str> = line.split(" ").collect();
                if vec.len() < 4 {
                    return None
                }
                return Some(Mount{
                    source: String::from(vec[0]),
                    target: String::from(vec[1]),
                    fstype: String::from(vec[2]),
                    flags:  String::from(vec[3]),
                })
            }
        }
    }
}

pub struct Filesystem {
    root : String
}

impl Filesystem {

    pub fn new(root: String) -> Filesystem {
        return Filesystem{root: root}
    }

    pub fn bind(&self, host: &str , container: String) {
        let hostcpath = self.root.clone() + "/" + &*container;
        create_dir_all(hostcpath.clone()).unwrap();

        mount(Some(host), &*hostcpath, None as Option<&str>, MS_BIND | MS_REC , None as Option<&str>)
            .expect(&*(String::from("cannot bind mount ") + &*container));

        mount(None as Option<&str> , &*hostcpath, None as Option<&str>, MS_SLAVE, None as Option<&str>)
            .expect("cannot mark mount as slave");
    }

    pub fn mount(&self, host: Option<&str>, container: String, fstype: Option<&str>) {
        let hostcpath = self.root.clone() + "/" + &*container;

        println!("mount {} ", hostcpath);

        create_dir_all(hostcpath.clone()).unwrap();

        mount(host, &*hostcpath, fstype, MsFlags::empty(), None as Option<&str>)
            .expect(&*(String::from("cannot mount ") + &*container));
    }

    pub fn pivot(&self) -> errors::Result<()> {
        //TODO everyone is using pivot_root. not sure why, chroot should be the same here.
        chroot(&*self.root)?;
        chdir("/");
        Ok(())
    }

    pub fn clear(&self) -> errors::Result<()> {
        //TODO instead of repeating, it should probably be recursive
        let mut tries = 0;
        loop {
            tries += 1;

            let mut complete = true;

            let mut path_vec : Vec<&str> = self.root.split("/").filter(|s|!s.is_empty()).collect();
            path_vec.insert(0, "");
            let clean_path = &*path_vec.join("/");

            for m in mounts().unwrap() {
                if m.target.starts_with(clean_path) {
                    match umount(&*m.target) {
                        Ok(_)   => {},
                        Err(e)  => {
                            complete = false;
                            if tries > 3 {
                                println!("unmount {} didn't work. giving up", m.target);
                                Err(e)?;
                            }
                        },
                    }
                }
            }
            if complete {
                return Ok(())
            }
        }
    }
}




