//! Repository source admission and identity extraction.

use percent_encoding::percent_decode_str;
use url::Url;

/// A selected clone address and repository identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitSource {
    /// The selected clone address.
    pub repo: String,
    /// The repository host.
    pub host: String,
    /// The repository path without a terminal `.git`.
    pub path: String,
    /// The selected branch, tag or commit.
    pub r#ref: Option<String>,
    /// Whether a nonempty ref was selected.
    pub pinned: bool,
}

/// Parses an admitted repository source, returning `None` for rejected forms.
#[must_use]
pub fn parse_git_url(source: &str) -> Option<GitSource> {
    let trimmed = crate::trim(source);
    let prefix = trimmed
        .strip_prefix("git:")
        .filter(|_| !trimmed.starts_with("git://"));
    let source = crate::trim(prefix.unwrap_or(trimmed));
    if prefix.is_none() && !explicit_protocol(source) {
        return None;
    }
    let (repo, reference) = split_ref(source);
    let first = reference
        .as_ref()
        .map(|reference| format!("{repo}#{reference}"));
    let https_first = first
        .as_ref()
        .map(|candidate| format!("https://{candidate}"));
    let https_original = format!("https://{source}");
    for candidate in first
        .as_deref()
        .into_iter()
        .chain([source])
        .chain(https_first.as_deref())
        .chain([https_original.as_str()])
    {
        if let Some(mut selected) = hosted(candidate) {
            if reference.is_some()
                && selected
                    .path
                    .rsplit('/')
                    .next()
                    .is_some_and(|project| project.contains('@'))
            {
                continue;
            }
            selected.r#ref = selected.r#ref.or_else(|| reference.clone());
            selected.pinned = selected.r#ref.is_some();
            selected.repo = clone_address(&repo, &selected)?;
            return Some(selected);
        }
    }
    generic(&repo, reference)
}

/// Whether a spelling starts with an admitted real protocol.
fn explicit_protocol(source: &str) -> bool {
    source.split_once("://").is_some_and(|(scheme, _)| {
        matches!(
            scheme.to_ascii_lowercase().as_str(),
            "http" | "https" | "ssh" | "git"
        )
    })
}

/// Separates the first nonempty path ref without treating credentials as a ref.
fn split_ref(source: &str) -> (String, Option<String>) {
    if source.contains("://") {
        if let Ok(mut url) = Url::parse(source)
            && let Some((path, reference)) = ref_parts(url.path().trim_start_matches('/'))
        {
            let path = path.to_owned();
            let reference = reference.to_owned();
            url.set_path(&format!("/{path}"));
            return (
                url.as_str().trim_end_matches('/').to_owned(),
                Some(reference),
            );
        }
    } else {
        let boundary = if source.starts_with("git@") {
            source.find(':')
        } else {
            source.find('/')
        };
        if let Some(boundary) = boundary
            && let Some((path, reference)) = ref_parts(&source[boundary + 1..])
        {
            return (
                format!("{}{path}", &source[..=boundary]),
                Some(reference.to_owned()),
            );
        }
    }
    (source.to_owned(), None)
}

/// A path ref exists only when both halves are nonempty.
fn ref_parts(path: &str) -> Option<(&str, &str)> {
    path.split_once('@')
        .filter(|(path, reference)| !path.is_empty() && !reference.is_empty())
}

/// Extracts generic components without widening shorthand admission.
fn generic(repo: &str, reference: Option<String>) -> Option<GitSource> {
    let (clone, host, path) = if repo.starts_with("git@") {
        let (host, path) = repo.strip_prefix("git@")?.split_once(':')?;
        let parsed = git_url_parse::GitUrl::parse(repo).ok()?;
        (
            repo.to_owned(),
            parsed.host().unwrap_or(host).to_owned(),
            path.to_owned(),
        )
    } else if explicit_protocol(repo) {
        let url = Url::parse(repo).ok()?;
        (
            repo.to_owned(),
            url.host_str()?.to_owned(),
            url.path().trim_start_matches('/').to_owned(),
        )
    } else {
        let (host, path) = repo.split_once('/')?;
        if !host.contains('.') && host != "localhost" {
            return None;
        }
        (format!("https://{repo}"), host.to_owned(), path.to_owned())
    };
    let path = path
        .strip_suffix(".git")
        .unwrap_or(&path)
        .trim_start_matches('/');
    if host.is_empty() || path.is_empty() || path.split('/').count() < 2 {
        return None;
    }
    Some(GitSource {
        repo: clone,
        host,
        path: path.to_owned(),
        pinned: reference.is_some(),
        r#ref: reference,
    })
}

/// The provider that owns hosted path selection.
#[derive(Clone, Copy)]
enum Provider {
    /// Two-segment repositories and optional tree paths.
    Github,
    /// Nested group paths.
    Gitlab,
    /// Two-segment repositories except archive downloads.
    Bitbucket,
    /// Gist ids, with optional user identity.
    Gist,
    /// Two-segment repositories except archives.
    Sourcehut,
}

impl Provider {
    /// Finds a provider by its canonical host or shortcut.
    fn find(name: &str) -> Option<Self> {
        match name {
            "github" | "github.com" => Some(Self::Github),
            "gitlab" | "gitlab.com" => Some(Self::Gitlab),
            "bitbucket" | "bitbucket.org" => Some(Self::Bitbucket),
            "gist" | "gist.github.com" => Some(Self::Gist),
            "sourcehut" | "git.sr.ht" => Some(Self::Sourcehut),
            _ => None,
        }
    }
    /// The canonical hosted domain.
    fn domain(self) -> &'static str {
        match self {
            Self::Github => "github.com",
            Self::Gitlab => "gitlab.com",
            Self::Bitbucket => "bitbucket.org",
            Self::Gist => "gist.github.com",
            Self::Sourcehut => "git.sr.ht",
        }
    }
    /// The protocols accepted by this provider's hosted extraction.
    fn supports(self, scheme: &str) -> bool {
        match self {
            Self::Github => matches!(
                scheme,
                "git" | "http" | "git+ssh" | "git+https" | "ssh" | "https"
            ),
            Self::Gist => matches!(scheme, "git" | "git+ssh" | "git+https" | "ssh" | "https"),
            Self::Sourcehut => matches!(scheme, "git+ssh" | "https"),
            Self::Gitlab | Self::Bitbucket => {
                matches!(scheme, "git+ssh" | "git+https" | "ssh" | "https")
            }
        }
    }
    /// Selects the surviving user, repository and ref from a hosted URL.
    fn extract(self, url: &Url) -> Option<(String, String, String)> {
        let path = url.path().strip_prefix('/')?;
        let parts: Vec<_> = path.split('/').collect();
        let hash = url.fragment().unwrap_or_default();
        if matches!(self, Self::Gitlab) {
            if path.contains("/-/") || path.contains("/archive.tar.gz") {
                return None;
            }
            let (project, user) = parts.split_last()?;
            return Some((user.join("/"), (*project).to_owned(), hash.to_owned()));
        }
        let user = *parts.first()?;
        let project = parts.get(1).copied().unwrap_or_default();
        let aux = parts.get(2).copied().unwrap_or_default();
        let reference = match self {
            Self::Github if !aux.is_empty() => {
                if aux != "tree" {
                    return None;
                }
                parts.get(3).copied().unwrap_or_default()
            }
            Self::Bitbucket if aux == "get" => return None,
            Self::Sourcehut if aux == "archive" => return None,
            Self::Gist if aux == "raw" => return None,
            _ => hash,
        };
        if matches!(self, Self::Gist) && project.is_empty() {
            return Some(("null".to_owned(), user.to_owned(), reference.to_owned()));
        }
        Some((user.to_owned(), project.to_owned(), reference.to_owned()))
    }
}

/// Recognizes the dependency's two-component hosted shorthand.
fn github_shorthand(source: &str) -> bool {
    let before_hash = source.split('#').next().unwrap_or_default();
    !before_hash.starts_with(['.', '/', '@'])
        && !before_hash.ends_with('/')
        && before_hash.split('/').count() == 2
        && !before_hash.contains([':', '@'])
        && !before_hash
            .chars()
            .any(|c| crate::trim(&c.to_string()).is_empty())
}

/// Decodes hosted identity only when every percent escape is valid UTF-8.
fn decode(text: &str) -> Option<String> {
    for (index, _) in text.match_indices('%') {
        let escaped = text.as_bytes().get(index + 1..index + 3)?;
        if !escaped.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
    }
    percent_decode_str(text)
        .decode_utf8()
        .ok()
        .map(std::borrow::Cow::into_owned)
}

/// Selects hosted identity; a failed selection permits generic fallback.
fn hosted(source: &str) -> Option<GitSource> {
    let scp = source
        .starts_with("git@")
        .then(|| format!("ssh://{}", source.replacen(':', "/", 1)));
    let source = scp.as_deref().unwrap_or(source);
    let shorthand = github_shorthand(source).then(|| format!("github:{source}"));
    let source = shorthand.as_deref().unwrap_or(source);
    let (scheme, tail) = source.split_once(':')?;
    let shortcut = (!tail.starts_with("//")).then_some(());
    let (provider, user, project, reference) = if shortcut.is_some() {
        let provider = Provider::find(scheme)?;
        let (path, hash) = tail.split_once('#').unwrap_or((tail, ""));
        let path = path.strip_prefix('/').unwrap_or(path);
        let path = path.split_once('@').map_or(path, |(_, after)| after);
        let (user, project) = path.rsplit_once('/').unwrap_or(("null", path));
        (
            provider,
            user.to_owned(),
            project.to_owned(),
            hash.to_owned(),
        )
    } else {
        let url = Url::parse(source).ok()?;
        let provider = Provider::find(
            url.host_str()?
                .strip_prefix("www.")
                .unwrap_or(url.host_str()?),
        )?;
        if !provider.supports(url.scheme()) {
            return None;
        }
        let (user, project, reference) = provider.extract(&url)?;
        (provider, user, project, reference)
    };
    let user = decode(&user)?;
    let project = project.strip_suffix(".git").unwrap_or(&project);
    let project = decode(project)?;
    let reference = decode(&reference)?;
    if user.is_empty() || project.is_empty() {
        return None;
    }
    let path = format!("{user}/{project}");
    let path = path.strip_suffix(".git").unwrap_or(&path).to_owned();
    Some(GitSource {
        repo: String::new(),
        host: provider.domain().to_owned(),
        path,
        pinned: !reference.is_empty(),
        r#ref: (!reference.is_empty()).then_some(reference),
    })
}

/// Constructs the clone target from the selected repository, keeping explicit auth and ports.
fn clone_address(repo: &str, selected: &GitSource) -> Option<String> {
    let target = if selected.host == "gist.github.com" {
        selected.path.rsplit('/').next()?
    } else {
        &selected.path
    };
    if let Some((authority, path)) = repo
        .strip_prefix("git@")
        .and_then(|scp| scp.split_once(':'))
    {
        let identity = path.strip_suffix(".git").unwrap_or(path);
        return Some(if decode(identity).as_deref() == Some(target) {
            repo.to_owned()
        } else {
            format!("git@{authority}:{target}")
        });
    }
    let explicit = repo.contains("://");
    if explicit {
        let mut url = Url::parse(repo).ok()?;
        let scheme = url
            .scheme()
            .strip_prefix("git+")
            .unwrap_or(url.scheme())
            .to_owned();
        if scheme != url.scheme() {
            url = Url::parse(&repo.replacen("git+", "", 1)).ok()?;
        }
        let old_identity = url
            .path()
            .trim_start_matches('/')
            .strip_suffix(".git")
            .unwrap_or(url.path().trim_start_matches('/'));
        if decode(old_identity).as_deref() == Some(target)
            && url.fragment().is_none()
            && !repo.starts_with("git+")
        {
            return Some(repo.to_owned());
        }
        url.set_path(&format!("/{target}"));
        url.set_fragment(None);
        return Some(url.to_string());
    }
    Some(format!("https://{}/{target}", selected.host))
}
