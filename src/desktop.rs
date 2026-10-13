use crate::config::{Config, home};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug)]
pub struct App {
    pub id: String,
    pub name: String,
    pub description: String,
    pub keywords: String,
    pub icon: String,
    pub icon_path: Option<PathBuf>,
    exec: String,
    path: PathBuf,
    working_dir: Option<PathBuf>,
    terminal: bool,
}
pub fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".local/share")),
    ];
    dirs.extend(env::split_paths(
        &env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
    ));
    dirs
}
fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    walk_matching(dir, depth, out, &|_| true);
}
fn walk_matching(
    dir: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
    matches: &impl Fn(&Path) -> bool,
) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    // Filter before allocating paths and sorting whole themes. Keep directories
    // so icons in nonstandard theme layouts still work.
    let mut entries: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) || matches(Path::new(&e.file_name())))
        .collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk_matching(&e.path(), depth - 1, out, matches);
        } else {
            out.push(e.path());
        }
    }
}
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('s') => out.push(' '),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(c) => {
                    out.push('\\');
                    out.push(c);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
fn parse(s: &str, path: PathBuf, id: String, desktops: &[String]) -> Option<App> {
    let mut map = BTreeMap::new();
    let mut active = false;
    for line in s.lines().map(str::trim) {
        if line.starts_with('[') {
            active = line == "[Desktop Entry]";
            continue;
        }
        if active
            && !line.starts_with('#')
            && let Some((k, v)) = line.split_once('=')
        {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let get = |k: &str| map.get(k).map(String::as_str).unwrap_or("");
    if get("Type") != "Application" || get("Hidden") == "true" || get("NoDisplay") == "true" {
        return None;
    }
    let matches = |value: &str| {
        value
            .split(';')
            .filter(|d| !d.is_empty())
            .any(|d| desktops.iter().any(|s| s == d))
    };
    if (!get("OnlyShowIn").is_empty() && !matches(get("OnlyShowIn"))) || matches(get("NotShowIn")) {
        return None;
    }
    if !get("TryExec").is_empty() && !executable_exists(get("TryExec")) {
        return None;
    }
    let locale = env::var("LC_ALL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| env::var("LC_MESSAGES").ok())
        .or_else(|| env::var("LANG").ok())
        .unwrap_or_default();
    let locale = locale.split('.').next().unwrap_or("");
    let localized = |key: &str| {
        map.get(&format!("{key}[{locale}]"))
            .or_else(|| {
                map.get(&format!(
                    "{key}[{}]",
                    locale.split('_').next().unwrap_or("")
                ))
            })
            .or_else(|| map.get(key))
            .map(|s| unescape(s))
            .unwrap_or_default()
    };
    let name = localized("Name");
    if name.is_empty() || get("Exec").is_empty() {
        return None;
    }
    Some(App {
        id,
        name,
        description: localized("Comment"),
        keywords: localized("Keywords"),
        icon: unescape(get("Icon")),
        icon_path: None,
        exec: unescape(get("Exec")),
        path,
        working_dir: (!get("Path").is_empty()).then(|| PathBuf::from(unescape(get("Path")))),
        terminal: get("Terminal") == "true",
    })
}
fn executable_exists(name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let executable = |p: &Path| {
        fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if name.contains('/') {
        executable(Path::new(name))
    } else {
        env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .any(|p| executable(&p.join(name)))
    }
}
pub fn discover_entries() -> Vec<App> {
    let desktops: Vec<_> = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .map(str::to_owned)
        .collect();
    let mut seen = HashSet::new();
    let mut apps = vec![];
    for dir in data_dirs() {
        let root = dir.join("applications");
        let mut paths = vec![];
        walk(&root, 8, &mut paths);
        for path in paths {
            if path.extension().is_none_or(|e| e != "desktop") {
                continue;
            }
            let id = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('/', "-");
            // Hidden entries in higher-priority roots mask system entries too.
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Ok(s) = fs::read_to_string(&path)
                && let Some(app) = parse(&s, path, id, &desktops)
            {
                apps.push(app);
            }
        }
    }
    apps.sort_by_cached_key(|a| a.name.to_lowercase());
    apps
}
pub fn discover(config: &Config) -> Vec<App> {
    let mut apps = discover_entries();
    let wanted = apps
        .iter()
        .map(|app| app.icon.as_str())
        .filter(|icon| !icon.is_empty() && !Path::new(icon).is_absolute())
        .collect();
    let icons = icon_index(&config.icon_theme, wanted);
    for app in &mut apps {
        let p = PathBuf::from(&app.icon);
        app.icon_path = if p.is_absolute() && p.is_file() {
            Some(p)
        } else {
            icons.get(&app.icon).cloned()
        };
    }
    apps
}
fn icon_index(theme: &str, wanted: HashSet<&str>) -> HashMap<String, PathBuf> {
    let mut roots = vec![home().join(".icons")];
    roots.extend(data_dirs().into_iter().map(|p| p.join("icons")));
    icon_index_in(theme, &roots, &data_dirs(), wanted)
}
fn icon_index_in(
    theme: &str,
    roots: &[PathBuf],
    data_roots: &[PathBuf],
    mut wanted: HashSet<&str>,
) -> HashMap<String, PathBuf> {
    let mut themes = vec![theme.to_owned()];
    let mut visited = HashSet::new();
    let mut icons = HashMap::new();
    let mut i = 0;
    while i < themes.len() && i < 32 && !wanted.is_empty() {
        let name = themes[i].clone();
        i += 1;
        if !visited.insert(name.clone()) {
            if i == themes.len() && !visited.contains("hicolor") {
                themes.push("hicolor".into());
            }
            continue;
        }
        for root in roots {
            let dir = root.join(&name);
            if let Ok(s) = fs::read_to_string(dir.join("index.theme")) {
                for line in s.lines() {
                    if let Some(value) = line.strip_prefix("Inherits=") {
                        themes.extend(
                            value
                                .split(',')
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned),
                        );
                    }
                }
            }
            let mut paths = vec![];
            walk_matching(&dir, 6, &mut paths, &|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|name| wanted.contains(name))
            });
            // Prefer scalable and 48/64px app artwork over tiny symbolic variants.
            paths.sort_by_key(|p| {
                let s = p.to_string_lossy();
                if s.contains("scalable/apps") {
                    0
                } else if s.contains("48x48/apps") || s.contains("64x64/apps") {
                    1
                } else {
                    2
                }
            });
            for p in paths {
                add_icon(&mut icons, p);
            }
            wanted.retain(|name| !icons.contains_key(*name));
            if wanted.is_empty() {
                return icons;
            }
        }
        if i == themes.len() && !visited.contains("hicolor") {
            themes.push("hicolor".into());
        }
    }
    for dir in data_roots {
        if wanted.is_empty() {
            break;
        }
        let mut paths = vec![];
        walk_matching(&dir.join("pixmaps"), 2, &mut paths, &|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|name| wanted.contains(name))
        });
        for p in paths {
            add_icon(&mut icons, p);
        }
        wanted.retain(|name| !icons.contains_key(*name));
    }
    icons
}
fn add_icon(icons: &mut HashMap<String, PathBuf>, p: PathBuf) {
    if !matches!(
        p.extension().and_then(|s| s.to_str()),
        Some("png" | "svg" | "jpg" | "jpeg" | "ico")
    ) {
        return;
    }
    if let Some(name) = p.file_stem().and_then(|s| s.to_str()) {
        icons.entry(name.to_owned()).or_insert(p);
    }
}
/// Parse desktop Exec syntax without invoking a shell. Single quotes are literal.
fn tokens(s: &str) -> Result<Vec<String>, String> {
    let mut out = vec![];
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' => {
                let next = chars.next().ok_or("trailing backslash in Exec")?;
                word.push(next);
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err("unclosed quote in Exec".into());
    }
    if started {
        out.push(word);
    }
    Ok(out)
}
impl App {
    pub fn command(&self, config: &Config) -> Result<Vec<String>, String> {
        let mut args = vec![];
        for token in tokens(&self.exec)? {
            match token.as_str() {
                "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => {}
                "%i" => {
                    if !self.icon.is_empty() {
                        args.extend(["--icon".into(), self.icon.clone()]);
                    }
                }
                "%c" => args.push(self.name.clone()),
                "%k" => args.push(self.path.to_string_lossy().into_owned()),
                _ => {
                    let mut expanded = String::new();
                    let mut chars = token.chars();
                    while let Some(c) = chars.next() {
                        if c != '%' {
                            expanded.push(c);
                            continue;
                        }
                        match chars.next() {
                            Some('%') => expanded.push('%'),
                            _ => {
                                return Err(format!(
                                    "unsupported embedded field code in {token:?}"
                                ));
                            }
                        }
                    }
                    args.push(expanded);
                }
            }
        }
        if args.first().is_none_or(|s| s.is_empty()) {
            return Err("empty Exec command".into());
        }
        if self.terminal {
            let mut terminal = config.terminal.clone();
            terminal.extend(args);
            args = terminal;
        }
        Ok(args)
    }
    pub fn launch(&self, config: &Config) -> Result<(), String> {
        let args = self.command(config)?;
        let mut command = Command::new(&args[0]);
        command
            .args(&args[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dir) = &self.working_dir {
            command.current_dir(dir);
        }
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        command
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not launch {}: {e}", self.name))
    }
}
pub fn rank(apps: &[App], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return (0..apps.len()).collect();
    }
    let mut hits: Vec<_> = apps
        .iter()
        .enumerate()
        .filter_map(|(i, a)| {
            let name = a.name.to_lowercase();
            let score = score(&name, &query).map(|s| s + 200).or_else(|| {
                score(
                    &format!("{} {} {}", a.id, a.keywords, a.description).to_lowercase(),
                    &query,
                )
            });
            score.map(|s| (i, s))
        })
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    hits.into_iter().map(|(i, _)| i).collect()
}
fn score(haystack: &str, query: &str) -> Option<i32> {
    if haystack == query {
        return Some(1000);
    }
    if haystack.starts_with(query) {
        return Some(800 - haystack.chars().count() as i32);
    }
    if let Some(pos) = haystack.find(query) {
        return Some(600 - pos as i32);
    }
    let chars: Vec<_> = haystack.chars().collect();
    let mut cursor = 0;
    let mut score = 0;
    let mut previous = None;
    for needle in query.chars().filter(|c| !c.is_whitespace()) {
        let pos = (cursor..chars.len()).find(|&i| chars[i] == needle)?;
        score += if pos == 0 || !chars[pos - 1].is_alphanumeric() {
            30
        } else {
            5
        };
        if previous == Some(pos.wrapping_sub(1)) {
            score += 15;
        }
        score -= (pos - cursor) as i32;
        previous = Some(pos);
        cursor = pos + 1;
    }
    Some(score)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn app(exec: &str) -> App {
        parse(
            &format!("[Desktop Entry]\nType=Application\nName=Test App\nExec={exec}\nIcon=test\n"),
            "/tmp/test.desktop".into(),
            "test.desktop".into(),
            &[],
        )
        .unwrap()
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = env::temp_dir().join(format!(
                "blastoff-icons-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self, path: &str, content: &str) -> PathBuf {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn targeted_icons_preserve_theme_root_size_and_fallback_priority() {
        let fixture = Fixture::new();
        fixture.file("user/Preferred/index.theme", "Inherits=Parent,Preferred\n");
        fixture.file("user/Parent/index.theme", "Inherits=Preferred\n");
        fixture.file("user/Preferred/16x16/apps/editor.png", "");
        let editor = fixture.file("user/Preferred/scalable/apps/editor.svg", "");
        fixture.file("system/Preferred/scalable/apps/editor.svg", "");
        fixture.file("user/Parent/scalable/apps/editor.svg", "");
        let inherited = fixture.file("user/Parent/custom/layout/inherited.png", "");
        let fallback = fixture.file("system/hicolor/48x48/apps/fallback.png", "");
        let pixmap = fixture.file("data/pixmaps/pixmap.jpg", "");
        fixture.file("user/Preferred/scalable/apps/unused.svg", "");
        fixture.file("user/Preferred/scalable/apps/unsupported.gif", "");
        let roots = [fixture.0.join("user"), fixture.0.join("system")];
        let data_roots = [fixture.0.join("data")];
        let icons = icon_index_in(
            "Preferred",
            &roots,
            &data_roots,
            HashSet::from([
                "editor",
                "inherited",
                "fallback",
                "pixmap",
                "missing",
                "unsupported",
            ]),
        );
        assert_eq!(icons.len(), 4);
        assert_eq!(icons["editor"], editor);
        assert_eq!(icons["inherited"], inherited);
        assert_eq!(icons["fallback"], fallback);
        assert_eq!(icons["pixmap"], pixmap);
        assert!(icon_index_in("Preferred", &roots, &data_roots, HashSet::new()).is_empty());
    }
    #[test]
    fn exec_fields_and_quoted_paths() {
        let a = app("\"/opt/My App/bin\" --title %c %i %k %U %%");
        assert_eq!(
            a.command(&Config::default()).unwrap(),
            [
                "/opt/My App/bin",
                "--title",
                "Test App",
                "--icon",
                "test",
                "/tmp/test.desktop",
                "%"
            ]
        );
    }
    #[test]
    fn exec_does_not_evaluate_shell() {
        assert_eq!(
            app("app $(touch /tmp/nope) ; echo")
                .command(&Config::default())
                .unwrap(),
            ["app", "$(touch", "/tmp/nope)", ";", "echo"]
        );
    }
    #[test]
    fn desktop_escapes_and_terminal_arguments() {
        assert_eq!(unescape(r"A\\B\sC"), "A\\B C");
        let mut a = app(r#"app "literal\\\\path" "two words""#);
        a.terminal = true;
        let config = Config {
            terminal: vec!["terminal".into(), "--execute".into()],
            ..Config::default()
        };
        assert_eq!(
            a.command(&config).unwrap(),
            ["terminal", "--execute", "app", "literal\\path", "two words"]
        );
    }
    #[test]
    fn rejects_invalid_exec() {
        assert!(app("app \"broken").command(&Config::default()).is_err());
        assert!(app("app --file=%f").command(&Config::default()).is_err());
    }
    #[test]
    fn hidden_and_other_desktops_are_filtered() {
        for extra in [
            "Hidden=true",
            "NoDisplay=true",
            "OnlyShowIn=GNOME;",
            "NotShowIn=sway;",
        ] {
            assert!(
                parse(
                    &format!("[Desktop Entry]\nType=Application\nName=A\nExec=a\n{extra}"),
                    "a.desktop".into(),
                    "a".into(),
                    &["sway".into()]
                )
                .is_none()
            );
        }
    }
    #[test]
    fn unset_desktop_does_not_hide_unrestricted_apps() {
        for extra in ["", "NotShowIn=", "NotShowIn=GNOME;", "OnlyShowIn="] {
            assert!(
                parse(
                    &format!("[Desktop Entry]\nType=Application\nName=A\nExec=a\n{extra}"),
                    "a.desktop".into(),
                    "a".into(),
                    &[String::new()]
                )
                .is_some()
            );
        }
        assert!(
            parse(
                "[Desktop Entry]\nType=Application\nName=A\nExec=a\nOnlyShowIn=GNOME;",
                "a.desktop".into(),
                "a".into(),
                &[String::new()]
            )
            .is_none()
        );
    }
    #[test]
    fn actions_do_not_override_main_entry() {
        let a = parse("[Desktop Entry]\nType=Application\nName=Main\nExec=main\n[Desktop Action New]\nName=Other\nExec=other", "a.desktop".into(), "a".into(), &[]).unwrap();
        assert_eq!(a.name, "Main");
    }
    #[test]
    fn fuzzy_unicode_and_ordering() {
        assert!(score("visual studio code", "vsc").is_some());
        assert!(score("éclair", "écl").is_some());
        assert!(score("firefox", "zz").is_none());
        let apps = vec![app("foo"), {
            let mut a = app("bar");
            a.name = "Test".into();
            a
        }];
        assert_eq!(rank(&apps, "test"), vec![1, 0]);
    }
}
