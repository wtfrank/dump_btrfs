use anyhow::*;
use libc::c_void;
use more_asserts::*;
use std::fs::File;
use std::ops::Index;
use std::os::fd::AsRawFd;
use std::path::Path;

/// Interpret offsets of a memory mapped file as
/// references to arbitrary types.

pub struct MappedFile {
    pointer: *mut c_void,
    len: usize,
    mapping_size: usize,
}

impl MappedFile {
    pub fn open(file: &Path) -> Result<MappedFile> {
        let f = File::open(file)?;
        let md = f.metadata()?;
        let len = if md.is_file() {
            md.len() as usize
        } else {
            //assume block device
            let mut len64 = 0_u64;
            let len_ref = &mut len64 as *mut u64;
            let ret = unsafe { ioctls::blkgetsize64(f.as_raw_fd(), len_ref) };
            assert_eq!(0, ret);
            len64 as usize
        };
        let ps = sysconf::page::pagesize();
        let mapping_size = ((len + ps - 1) / ps) * ps;
        let p = unsafe {
            libc::mmap(
                0 as *mut c_void,
                len,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                f.as_raw_fd(),
                0,
            )
        };
        if libc::MAP_FAILED == p {
            return Err(anyhow!(
                "Failed to map file: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(MappedFile {
            pointer: p,
            len,
            mapping_size,
        })
    }

    /// Returns a reference to T. T should be a primitive type or
    /// (probably) #[repr(C)]
    /// panics if the index is out of bounds.
    pub fn at<T>(&self, offset: usize) -> &T {
        if self.len - std::mem::size_of::<T>() <= offset {
            panic!("access beyond end of file");
        }
        unsafe { &*((self.pointer as usize + offset) as *mut c_void as *const T) }
    }

    /// Returns a slice of u8s representing part of the mapped file
    pub fn slice(&self, offset: usize, length: usize) -> &[u8] {
        assert_le!(offset + length, self.len);
        unsafe {
            std::slice::from_raw_parts(
                &*((self.pointer as usize + offset) as *mut c_void as *const u8),
                length,
            )
        }
    }
}

impl Drop for MappedFile {
    fn drop(&mut self) {
        unsafe {
            let ret = libc::munmap(self.pointer, self.mapping_size);
            assert_eq!(ret, 0);
        }
    }
}

impl Index<usize> for MappedFile {
    type Output = u8;

    fn index(&self, idx: usize) -> &Self::Output {
        if self.len - std::mem::size_of::<usize>() <= idx {
            panic!("access beyond end of file");
        }
        unsafe { &*((self.pointer as usize + idx) as *mut c_void as *const u8) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_index() -> Result<()> {
        let mf = MappedFile::open(&Path::new("Cargo.toml"))?;
        assert_eq!(mf[0], '[' as u8);
        assert_eq!(mf[1], 'p' as u8);
        assert_eq!(mf[2], 'a' as u8);
        assert_eq!(mf[3], 'c' as u8);
        assert_eq!(mf[4], 'k' as u8);
        assert_eq!(mf[0], '[' as u8);
        Ok(())
    }

    #[test]
    fn file_at() -> Result<()> {
        let mf = MappedFile::open(&Path::new("Cargo.toml"))?;
        assert_eq!(*mf.at::<u8>(0), '[' as u8);
        assert_eq!(*mf.at::<u8>(1), 'p' as u8);

        assert_eq!(*mf.at::<u16>(0), unsafe {
            std::mem::transmute::<[u8; 2], u16>(['[' as u8, 'p' as u8])
        });
        assert_eq!(*mf.at::<u16>(1), unsafe {
            std::mem::transmute::<[u8; 2], u16>(['p' as u8, 'a' as u8])
        });

        Ok(())
    }
    #[test]
    #[should_panic(expected = "access beyond end of file")]
    fn file_index_panic() -> () {
        let mf = MappedFile::open(&Path::new("Cargo.toml")).unwrap();
        mf[mf.len];
    }

    #[test]
    #[should_panic]
    fn file_at_panic() -> () {
        let mf = MappedFile::open(&Path::new("Cargo.toml")).unwrap();
        mf.at::<u8>(mf.len);
    }
}
