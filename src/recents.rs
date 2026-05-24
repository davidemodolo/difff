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
