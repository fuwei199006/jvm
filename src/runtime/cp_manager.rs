use crate::util;
use bytes::{Buf, Bytes};
use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek};
use std::path::{self, Path};
use std::sync::{Arc, Mutex};
use zip::ZipArchive;

lazy_static! {
    static ref CPM: Mutex<ClassPathManager> = { Mutex::new(ClassPathManager::new()) };
}

pub fn init() {
    lazy_static::initialize(&CPM);
}

pub fn find_class(name: &str) -> Result<ClassPathResult, io::Error> {
    util::sync_call_ctx(&CPM, |cpm| cpm.search_class(name))
}

#[derive(Copy, Clone, Debug)]
pub enum ClassSource {
    DIR,
    JAR,
}

#[derive(Debug)]
pub struct ClassPathResult(pub String, pub ClassSource, pub Vec<u8>);

struct ClassPathEntry(ClassSource, String);

struct ClassPathManager {
    runtime_class_path: Vec<ClassPathEntry>,
}

impl ClassPathManager {
    fn new() -> Self {
        Self {
            runtime_class_path: vec![],
        }
    }

    pub fn add_class_path(&mut self, path: &str) -> Result<(), io::Error> {
        let p = Path::new(path);
        if p.is_dir() {
            self.runtime_class_path
                .push(ClassPathEntry(ClassSource::DIR, path.to_string()));
        } else {
            let _ = File::open(p)?;
            self.runtime_class_path
                .push(ClassPathEntry(ClassSource::JAR, path.to_string()));
        }

        Ok(())
    }

    pub fn add_class_paths(&mut self, path: &str) {
        path.split(util::PATH_DELIMITER_STR).for_each(|p| {
            self.add_class_path(p);
        });
    }

    pub fn search_class(&self, name: &str) -> Result<ClassPathResult, io::Error> {
        let name = name.replace("/", util::PATH_SEP_STR);
        let name = name.replace(".", util::PATH_SEP_STR);

        for it in self.runtime_class_path.iter() {
            match it.0 {
                ClassSource::DIR => {
                    let mut p = String::from(&it.1);
                    p.push_str(util::PATH_SEP_STR);
                    p.push_str(&name);
                    p.push_str(".class");
                    match File::open(&p) {
                        Ok(mut f) => {
                            let mut v = Vec::with_capacity(f.metadata().unwrap().len() as usize);
                            f.read_to_end(&mut v);

                            return Ok(ClassPathResult(p, it.0, v));
                        }

                        _ => (),
                    }
                }

                ClassSource::JAR => {
                    let mut p = String::from(&name);
                    p.push_str(".class");

                    let f = File::open(&it.1)?;
                    let mut z = ZipArchive::new(f)?;
                    let mut zf = z.by_name(&p);

                    match zf {
                        Ok(mut zf) => {
                            let mut v = Vec::with_capacity(zf.size() as usize);
                            zf.read_to_end(&mut v);

                            return Ok(ClassPathResult(
                                it.1.clone(),
                                it.0,
                                v));
                        }

                        _ => (),
                    }
                }
            }
        }

        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Search class failed: {}", name),
        ));
    }

    pub fn size(&self) -> usize {
        self.runtime_class_path.len()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn t_basic_zip() {
        let f = "test/class_path_test.jar";
        let f = super::File::open(f).unwrap();
        let mut za = super::ZipArchive::new(f).unwrap();

        for i in 0..za.len() {
            let mut zf = za.by_index(i).unwrap();
            println!("{}", zf.name());
        }
    }

    #[test]
    fn t_replace_all() {
        let class = "java.lang.String";
        assert_eq!(class.replace(".", "/"), "java/lang/String");
    }

    #[test]
    fn t_add_cls_path() {
        let mut cpm = super::ClassPathManager::new();
        assert!(cpm.add_class_path("test/").is_ok());
        assert!(cpm.add_class_path("test_no_exist/").is_err());
        assert!(cpm
            .add_class_path("test/classloader/class_path_test.jar")
            .is_ok());
        assert!(cpm
            .add_class_path("test/classloader/class_path_test_no_exist.jar")
            .is_err());
    }

    #[test]
    fn t_add_cls_paths() {
        let mut cpm = super::ClassPathManager::new();
        cpm.add_class_paths("test/:test/classloader/class_path_test.jar");
        assert_eq!(2, cpm.size());
    }

    #[test]
    fn t_search_cls() {
        let mut cpm = super::ClassPathManager::new();
        let _ = cpm.add_class_path("test/classloader/class_path_test.jar");
        assert!(cpm.search_class("Foo").is_ok());
    }

    #[test]
    fn t_search_cls2() {
        let mut cpm = super::ClassPathManager::new();
        cpm.add_class_paths("test/:test/classloader/class_path_test.jar");
        assert!(cpm.search_class("Sample").is_ok());
        assert!(cpm.search_class("Foo").is_ok());
    }
}