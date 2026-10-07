#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Clone)]
pub(super) enum HostThrown {
    Json(String),
    Error(Box<crate::Error>),
}
#[cfg(any(test, target_arch = "wasm32"))]
pub(super) fn host_thrown(value: HostThrown) -> Result<ThrownValue, serde_json::Error> {
    match value {
        HostThrown::Json(text) => serde_json::from_str(&text).map(ThrownValue::Json),
        HostThrown::Error(error) => Ok(ThrownValue::Error(error)),
    }
}
use crate::ThrownValue;
pub(super) struct EnvironmentValue {
    truthy: bool,
    text: ThrownValue,
}
impl EnvironmentValue {
    pub(super) fn string(value: String) -> Self {
        Self {
            truthy: !value.is_empty(),
            text: ThrownValue::Json(value.into()),
        }
    }
    pub(super) fn is_truthy(&self) -> bool {
        self.truthy
    }
    pub(super) fn into_string(self) -> Result<String, ThrownValue> {
        crate::format_thrown_value(&self.text)
    }
}
#[cfg(any(test, target_arch = "wasm32"))]
pub(super) fn host_environment_value(truthy: bool, text: ThrownValue) -> EnvironmentValue {
    EnvironmentValue { truthy, text }
}

pub(super) trait Platform {
    fn facilities(&self) -> [bool; 3];
    fn native(&self) -> Result<bool, ThrownValue>;
    fn env(&self, key: &str) -> Result<Option<EnvironmentValue>, ThrownValue>;
    fn home(&self) -> Result<String, ThrownValue>;
    fn join(&self, home: &str) -> Result<String, ThrownValue>;
    fn exists(&self, path: &str) -> bool;
    fn bun(&self) -> Result<bool, ThrownValue>;
    fn own_count(&self) -> Result<usize, ThrownValue>;
    fn proc_bytes(&self) -> Option<Vec<u8>>;
}
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

    fn env(&self, key: &str) -> Result<Option<EnvironmentValue>, ThrownValue> {
        Ok(std::env::var_os(key)
            .map(|v| EnvironmentValue::string(v.to_string_lossy().into_owned())))
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

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use js_sys::{
        Reflect,
        wasm_bindgen::{JsCast, JsValue},
    };
    fn thrown(value: JsValue) -> ThrownValue {
        if let Some(error) = value.dyn_ref::<js_sys::Error>() {
            return host_thrown(HostThrown::Error(Box::new(crate::Error {
                name: error.name().into(),
                message: error.message().into(),
                stack: Reflect::get(&value, &"stack".into())
                    .ok()
                    .and_then(|v| v.as_string()),
                code: metadata(&value, "code"),
                errno: metadata(&value, "errno"),
                cause: metadata(&value, "cause"),
            })))
            .expect("Error metadata needs no JSON decoding");
        }
        if value.is_undefined() {
            ThrownValue::Undefined
        } else if let Some(number) = value.as_f64() {
            ThrownValue::Number(number)
        } else if let Some(text) = value.as_string() {
            ThrownValue::Json(text.into())
        } else if let Some(value) = value.as_bool() {
            ThrownValue::Json(value.into())
        } else if let Ok(Some(text)) = js_sys::JSON::stringify(&value).map(|text| text.as_string())
            && let Ok(payload) = host_thrown(HostThrown::Json(text))
        {
            payload
        } else {
            ThrownValue::StringCoercion(std::sync::Arc::new(move || string_value(&value)))
        }
    }
    fn string_value(value: &JsValue) -> Result<String, ThrownValue> {
        let constructor = property(&js_sys::global(), "String")?;
        let result = constructor
            .unchecked_ref::<js_sys::Function>()
            .call1(&JsValue::UNDEFINED, value)
            .map_err(thrown)?;
        Ok(String::from(result.unchecked_into::<js_sys::JsString>()))
    }
    fn metadata(value: &JsValue, name: &str) -> Option<ThrownValue> {
        Reflect::get(value, &name.into())
            .ok()
            .filter(|v| !v.is_undefined())
            .map(thrown)
    }
    fn property(target: &JsValue, name: &str) -> Result<JsValue, ThrownValue> {
        Reflect::get(target, &JsValue::from_str(name)).map_err(thrown)
    }
    fn process() -> Result<JsValue, ThrownValue> {
        let process = property(&js_sys::global(), "process")?;
        if process.is_undefined() {
            return Err(ThrownValue::Error(Box::new(crate::Error {
                name: "ReferenceError".into(),
                message: "process is not defined".into(),
                stack: None,
                code: None,
                errno: None,
                cause: None,
            })));
        }
        Ok(process)
    }
    fn version(process: &JsValue, name: &str) -> Result<bool, ThrownValue> {
        let versions = property(process, "versions")?;
        if versions.is_null() || versions.is_undefined() {
            return Ok(false);
        }
        Ok(property(&versions, name)?.is_truthy())
    }
    impl Platform for Host {
        fn facilities(&self) -> [bool; 3] {
            [false; 3]
        }
        fn native(&self) -> Result<bool, ThrownValue> {
            let process = property(&js_sys::global(), "process")?;
            if process.is_undefined() {
                return Ok(false);
            }
            Ok(version(&process, "node")? || version(&process, "bun")?)
        }
        fn env(&self, key: &str) -> Result<Option<EnvironmentValue>, ThrownValue> {
            let env = property(&process()?, "env")?;
            let value = property(&env, key)?;
            let truthy = value.is_truthy();
            Ok(Some(host_environment_value(
                truthy,
                ThrownValue::StringCoercion(std::sync::Arc::new(move || string_value(&value))),
            )))
        }
        fn bun(&self) -> Result<bool, ThrownValue> {
            version(&process()?, "bun")
        }
        fn own_count(&self) -> Result<usize, ThrownValue> {
            let env = property(&process()?, "env")?;
            let keys = Reflect::own_keys(&env).map_err(thrown)?;
            let mut count = 0;
            for key in keys.iter().filter(|k| k.is_string()) {
                let descriptor = Reflect::get_own_property_descriptor(
                    env.unchecked_ref::<js_sys::Object>(),
                    &key,
                )
                .map_err(thrown)?;
                if property(&descriptor, "enumerable")?.is_truthy() {
                    count += 1;
                }
            }
            Ok(count)
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
