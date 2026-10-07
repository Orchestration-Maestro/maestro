use crate::ThrownValue;
pub(super) trait Platform {
    fn facilities(&self) -> [bool; 3];
    fn native(&self) -> Result<bool, ThrownValue>;
    fn env(&self, key: &str) -> Result<Option<String>, ThrownValue>;
    fn home(&self) -> Result<String, ThrownValue>;
    fn join(&self, home: &str) -> Result<String, ThrownValue>;
    fn exists(&self, path: &str) -> bool;
    fn bun(&self) -> Result<bool, ThrownValue>;
    fn own_count(&self) -> Result<usize, ThrownValue>;
    fn proc_bytes(&self) -> Option<Vec<u8>>;
}
#[cfg(not(target_arch = "wasm32"))]
pub(super) struct Host;
#[cfg(not(target_arch = "wasm32"))]
impl Platform for Host {
    fn facilities(&self) -> [bool; 3] {
        [true; 3]
    }
    fn native(&self) -> Result<bool, ThrownValue> {
        Ok(true)
    }
    fn bun(&self) -> Result<bool, ThrownValue> {
        Ok(false)
    }
    fn own_count(&self) -> Result<usize, ThrownValue> {
        Ok(std::env::vars_os().count())
    }
    fn proc_bytes(&self) -> Option<Vec<u8>> {
        std::fs::read("/proc/self/environ").ok()
    }

    #[allow(deprecated)]
    fn home(&self) -> Result<String, ThrownValue> {
        std::env::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .ok_or_else(|| {
                crate::records::diagnostics::error("Unable to get home directory".into())
            })
    }
    fn join(&self, home: &str) -> Result<String, ThrownValue> {
        #[cfg(windows)]
        {
            Ok(join_windows(home))
        }
        #[cfg(not(windows))]
        {
            Ok(join_posix(home))
        }
    }
    fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    fn env(&self, key: &str) -> Result<Option<String>, ThrownValue> {
        Ok(std::env::var_os(key).map(|v| v.to_string_lossy().into_owned()))
    }
}

#[cfg(any(test, all(not(target_arch = "wasm32"), not(windows))))]
pub(super) fn join_posix(home: &str) -> String {
    let full = format!("{home}/.config/gcloud/application_default_credentials.json");
    let absolute = !home.is_empty() && full.starts_with('/');
    let mut parts = Vec::new();
    for part in full.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|s| *s != "..") => {
                parts.pop();
            }
            ".." if !absolute => parts.push(part),
            ".." => {}
            _ => parts.push(part),
        }
    }
    format!("{}{}", if absolute { "/" } else { "" }, parts.join("/"))
}

#[cfg(any(test, target_arch = "wasm32"))]
pub(super) struct Browser;
#[cfg(target_arch = "wasm32")]
pub(super) use Browser as Host;
#[cfg(any(test, target_arch = "wasm32"))]
impl Platform for Browser {
    fn facilities(&self) -> [bool; 3] {
        [false; 3]
    }
    fn native(&self) -> Result<bool, ThrownValue> {
        Ok(false)
    }
    fn env(&self, _: &str) -> Result<Option<String>, ThrownValue> {
        Err(missing_process())
    }
    fn bun(&self) -> Result<bool, ThrownValue> {
        Err(missing_process())
    }
    fn own_count(&self) -> Result<usize, ThrownValue> {
        unreachable!("browser has no process")
    }
    fn proc_bytes(&self) -> Option<Vec<u8>> {
        None
    }
    fn exists(&self, _: &str) -> bool {
        false
    }
    fn home(&self) -> Result<String, ThrownValue> {
        unreachable!("browser has no native home facility")
    }
    fn join(&self, _: &str) -> Result<String, ThrownValue> {
        unreachable!("browser has no native path facility")
    }
}
#[cfg(any(test, target_arch = "wasm32"))]
fn missing_process() -> ThrownValue {
    ThrownValue::Error(Box::new(crate::Error {
        name: "ReferenceError".into(),
        message: "process is not defined".into(),
        stack: None,
        code: None,
    }))
}

#[cfg(any(test, windows))]
pub(super) fn join_windows(home: &str) -> String {
    let home = home.replace('/', "\\");
    let full = if home.is_empty() {
        ".config\\gcloud\\application_default_credentials.json".into()
    } else {
        format!("{home}\\.config\\gcloud\\application_default_credentials.json")
    };
    let (root, tail, absolute) = if home.starts_with("\\\\")
        && home.as_bytes().get(2).is_some_and(|b| *b != b'\\')
    {
        let mut parts = full.split('\\').filter(|s| !s.is_empty());
        let server = parts.next().unwrap_or("");
        let share = parts.next().unwrap_or("");
        (
            format!("\\\\{server}\\{share}\\"),
            parts.collect::<Vec<_>>().join("\\"),
            true,
        )
    } else if full.as_bytes().get(1) == Some(&b':') && full.as_bytes()[0].is_ascii_alphabetic() {
        let absolute = full[2..].starts_with('\\');
        (
            format!("{}{}", &full[..2], if absolute { "\\" } else { "" }),
            full[2..].to_owned(),
            absolute,
        )
    } else if full.starts_with('\\') {
        ("\\".into(), full.clone(), true)
    } else {
        (String::new(), full.clone(), false)
    };
    let mut parts = Vec::new();
    for part in tail.split('\\') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|s| *s != "..") => {
                parts.pop();
            }
            ".." if !absolute => parts.push(part),
            ".." => {}
            _ => parts.push(part),
        }
    }
    format!("{root}{}", parts.join("\\"))
}
