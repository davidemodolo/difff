use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RecentPair {
    pub left: String,
    pub right: String,
    pub left_name: String,
    pub right_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct Recents {
    pairs: Vec<RecentPair>,
    file_path: PathBuf,
}

impl Recents {
    pub fn load() -> Self {
        let file_path = data_dir().join("difff").join("recents");
        Self::from_file(file_path)
    }

    #[cfg(test)]
    pub fn from_file(file_path: PathBuf) -> Self {
        let pairs = fs::read_to_string(&file_path)
            .map(|s| parse_recents(&s))
            .unwrap_or_default();
        Self { pairs, file_path }
    }

    #[cfg(not(test))]
    fn from_file(file_path: PathBuf) -> Self {
        let pairs = fs::read_to_string(&file_path)
            .map(|s| parse_recents(&s))
            .unwrap_or_default();
        Self { pairs, file_path }
    }

    pub fn list(&self) -> &[RecentPair] {
        &self.pairs
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn add(&mut self, left: &str, right: &str) {
        self.pairs.retain(|p| p.left != left || p.right != right);
        self.pairs.insert(
            0,
            RecentPair {
                left: left.to_string(),
                right: right.to_string(),
                left_name: basename(left),
                right_name: basename(right),
            },
        );
        self.pairs.truncate(10);
        self.save();
    }

    fn save(&self) {
        let dir = self.file_path.parent().unwrap();
        let _ = fs::create_dir_all(dir);
        let content: String = self
            .pairs
            .iter()
            .flat_map(|p| [p.left.as_str(), p.right.as_str(), ""])
            .collect::<Vec<_>>()
            .join("\n");
        let _ = fs::write(&self.file_path, content);
    }
}

fn parse_recents(raw: &str) -> Vec<RecentPair> {
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    lines
        .chunks(2)
        .filter_map(|chunk| {
            if chunk.len() == 2 {
                Some(RecentPair {
                    left: chunk[0].to_string(),
                    right: chunk[1].to_string(),
                    left_name: basename(chunk[0]),
                    right_name: basename(chunk[1]),
                })
            } else {
                None
            }
        })
        .collect()
}

fn basename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(dir)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local").join("share")
    } else {
        PathBuf::from(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("difff-test").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("recents")
    }

    #[test]
    fn parse_two_pairs() {
        let input = "/a.py\n/b.py\n\n/c.py\n/d.py\n";
        let pairs = parse_recents(input);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].left, "/a.py");
        assert_eq!(pairs[0].right, "/b.py");
        assert_eq!(pairs[0].left_name, "a.py");
        assert_eq!(pairs[1].left, "/c.py");
        assert_eq!(pairs[1].right, "/d.py");
    }

    #[test]
    fn parse_empty() {
        assert!(parse_recents("").is_empty());
        assert!(parse_recents("\n\n\n").is_empty());
    }

    #[test]
    fn parse_odd_lines_ignored() {
        let pairs = parse_recents("/a.py\n/b.py\n/c.py\n");
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn add_new_pair_saves_and_reloads() {
        let path = test_path("add_reload");

        let mut r = Recents::from_file(path.clone());
        assert!(r.is_empty());

        r.add("/home/u/a.py", "/home/u/b.py");
        assert_eq!(r.list().len(), 1);

        r.add("/home/u/c.py", "/home/u/d.py");
        assert_eq!(r.list().len(), 2);
        assert_eq!(r.list()[0].left, "/home/u/c.py");
        assert_eq!(r.list()[1].left, "/home/u/a.py");

        let r2 = Recents::from_file(path);
        assert_eq!(r2.list().len(), 2, "reloaded file should have 2 pairs");
        assert_eq!(r2.list()[0].left, "/home/u/c.py");
    }

    #[test]
    fn add_duplicate_moves_to_top() {
        let path = test_path("dup_moves");

        let mut r = Recents::from_file(path);
        r.add("/x.py", "/y.py");
        r.add("/a.py", "/b.py");
        r.add("/x.py", "/y.py"); // duplicate → moves to top

        assert_eq!(r.list().len(), 2);
        assert_eq!(r.list()[0].left, "/x.py");
    }

    #[test]
    fn truncates_to_10() {
        let path = test_path("truncate");

        let mut r = Recents::from_file(path);
        for i in 0..12 {
            r.add(&format!("/{i}l.py"), &format!("/{i}r.py"));
        }
        assert_eq!(r.list().len(), 10);
        assert_eq!(r.list()[0].left, "/11l.py");
        assert_eq!(r.list()[9].left, "/2l.py");
    }

    #[test]
    fn basename_extracts_filename() {
        assert_eq!(basename("/home/u/file.txt"), "file.txt");
        assert_eq!(basename("file.txt"), "file.txt");
        assert_eq!(basename("/"), "/"); // root has no filename → returns path
    }
}
