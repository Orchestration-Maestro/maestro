use maestro_resources::*;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Controlled {
    pub files: BTreeMap<String, Result<String, ResourceError>>,
    pub dirs: BTreeMap<String, Result<Vec<Dirent>, ResourceError>>,
    pub stats: BTreeMap<String, Result<Stats, ResourceError>>,
    pub real: BTreeMap<String, Result<String, ResourceError>>,
    pub cwd_error: bool,
}
impl Controlled {
    pub fn file(&mut self, path: &str, content: &str) {
        self.files.insert(path.into(), Ok(content.into()));
    }
    pub fn dir(&mut self, path: &str, names: &[(&str, char)]) {
        self.dirs.insert(
            path.into(),
            Ok(names
                .iter()
                .map(|(n, k)| Dirent {
                    name: (*n).into(),
                    is_file: *k == 'f',
                    is_directory: *k == 'd',
                    is_symbolic_link: *k == 'l',
                })
                .collect()),
        );
    }
    pub fn skill(content: &str) -> Self {
        let mut s = Self::default();
        s.dir("/parent", &[("SKILL.md", 'f')]);
        s.file("/parent/SKILL.md", content);
        s
    }
}
impl ResourceOperations for Controlled {
    fn exists(&self, p: &str) -> bool {
        self.files.contains_key(p) || self.dirs.contains_key(p) || self.stats.contains_key(p)
    }
    fn read_dir(&self, p: &str) -> Result<Vec<Dirent>, ResourceError> {
        self.dirs
            .get(p)
            .cloned()
            .unwrap_or(Err(ResourceError { message: None }))
    }
    fn read_file(&self, p: &str) -> Result<String, ResourceError> {
        self.files
            .get(p)
            .cloned()
            .unwrap_or(Err(ResourceError { message: None }))
    }
    fn stat(&self, p: &str) -> Result<Stats, ResourceError> {
        self.stats.get(p).cloned().unwrap_or_else(|| {
            Ok(Stats {
                is_file: self.files.contains_key(p),
                is_directory: self.dirs.contains_key(p),
            })
        })
    }
    fn realpath(&self, p: &str) -> Result<String, ResourceError> {
        self.real.get(p).cloned().unwrap_or_else(|| Ok(p.into()))
    }
    fn current_dir(&self) -> Result<String, ResourceError> {
        if self.cwd_error {
            Err(ResourceError {
                message: Some("cwd failed".into()),
            })
        } else {
            Ok("/process".into())
        }
    }
}
