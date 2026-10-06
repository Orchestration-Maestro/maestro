use std::ffi::OsString;
use std::path::Path;

pub(super) fn run(args: Vec<OsString>) -> Result<u8, String> {
    if args.len() != 3 {
        return Err("repository tools: expected asset layout, source and destination".into());
    }
    let source = Path::new(&args[1]);
    let destination = Path::new(&args[2]);
    match args[0].to_str() {
        Some("library") => {
            group(
                source,
                destination,
                "src/modes/interactive/theme",
                "modes/interactive/theme",
                "json",
            )?;
            group(
                source,
                destination,
                "src/modes/interactive/assets",
                "modes/interactive/assets",
                "png",
            )?;
            for file in ["template.html", "template.css", "template.js"] {
                copy(
                    &source.join("src/core/export-html").join(file),
                    &destination.join("core/export-html").join(file),
                )?;
            }
            group(
                source,
                destination,
                "src/core/export-html/vendor",
                "core/export-html/vendor",
                "js",
            )?;
        }
        Some("standalone") => {
            for file in ["package.json", "README.md", "CHANGELOG.md"] {
                copy(&source.join(file), &destination.join(file))?;
            }
            group(
                source,
                destination,
                "src/modes/interactive/theme",
                "theme",
                "json",
            )?;
            group(
                source,
                destination,
                "src/modes/interactive/assets",
                "assets",
                "png",
            )?;
            copy(
                &source.join("src/core/export-html/template.html"),
                &destination.join("export-html/template.html"),
            )?;
            group(
                source,
                destination,
                "src/core/export-html/vendor",
                "export-html/vendor",
                "js",
            )?;
            for directory in ["docs", "examples"] {
                tree(&source.join(directory), &destination.join(directory))?;
            }
        }
        _ => return Err("repository tools: unknown asset layout".into()),
    }
    Ok(0)
}

fn copy(source: &Path, destination: &Path) -> Result<(), String> {
    let bytes = std::fs::read(source)
        .map_err(|error| format!("repository tools: asset {}: {error}", source.display()))?;
    std::fs::create_dir_all(
        destination
            .parent()
            .ok_or("repository tools: asset destination has no parent")?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(destination, bytes).map_err(|error| error.to_string())
}

fn group(
    source: &Path,
    destination: &Path,
    input: &str,
    output: &str,
    extension: &str,
) -> Result<(), String> {
    let directory = source.join(input);
    let mut count = 0;
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| format!("repository tools: asset {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .path()
            .extension()
            .is_some_and(|value| value == extension)
        {
            copy(
                &entry.path(),
                &destination.join(output).join(entry.file_name()),
            )?;
            count += 1;
        }
    }
    if count == 0 {
        return Err(format!(
            "repository tools: missing assets: {input}/*.{extension}"
        ));
    }
    Ok(())
}

fn tree(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in std::fs::read_dir(source)
        .map_err(|error| format!("repository tools: asset {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            tree(&entry.path(), &destination.join(entry.file_name()))?;
        } else {
            copy(&entry.path(), &destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}
