extern crate git2;
extern crate serde;

extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use git2::Repository;
use git2::*;

use std::path::Path;

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Default)]
struct Crate {
    name: String,
    vers: String,
    deps: Vec<Dependency>,
    cksum: String,
    features: HashMap<String, Vec<String>>,
    yanked: bool
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Dependency {
    name: String,
    req: String,
    features: Vec<String>,
    optional: bool,
    default_features: bool,
    target: Option<String>,
    kind: String
}

pub struct CratesIndex {
    repo: Repository
}

pub struct IndexIter<'a> {
    repo: &'a Repository,
    revwalk: Revwalk<'a>
}

#[derive(Debug)]
pub enum ChangeType {
    Modified,
    Deleted
}

#[derive(Debug)]
pub struct Change {
    delta: Delta,
    message: Option<String>,
    payload: Crate
}

impl Default for Change {
    fn default() -> Change {
        Change { delta: Delta::Modified, message: None, payload: Crate::default() }
    }
}

impl CratesIndex {
    pub fn new<P: AsRef<Path>>(p: P) -> Result<CratesIndex, Error> {
        let path = p.as_ref();
        Repository::open(path)
        .and_then(|repo| {
            Ok(CratesIndex { repo: repo })
        })
    }

    pub fn iter<'a>(&'a self) -> Result<IndexIter<'a>, Error> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(git2::SORT_REVERSE);
        revwalk.push_head()?;

        revwalk.next(); // Skip the first commit

        Ok(IndexIter { repo: &self.repo, revwalk })
    }
}

impl<'a> IndexIter<'a> {
    fn get_last_diff(&self, commit: &Commit) -> Result<Diff, Error> {
        let tree = self.repo.find_tree(commit.tree_id())?;

        let parent = commit.parent(0)?;

        let parent_tree = self.repo.find_tree(parent.tree_id())?;
        self.repo.diff_tree_to_tree(Some(&tree), Some(&parent_tree), None)
    }

    fn find_next_rev(&mut self) -> Option<Result<Oid, Error>> {
        let repo = self.repo;

        self.revwalk.find(|walk| {
            let rev = match walk {
                &Ok(r) => r,
                &Err(ref e) => return true,
            };

            let commit = match repo.find_commit(rev) {
                Ok(commit) => commit,
                Err(e) => return false,
            };

            if commit.author().name() == Some("bors") {
                true
            } else {
                false
            }
        })
    }
}

impl<'a> Iterator for IndexIter<'a> {
    type Item = Result<Change, Error>;

    fn next(&mut self) -> Option<Result<Change, Error>> {
        let rev = match self.find_next_rev() {
            Some(Ok(r)) => r,
            Some(Err(e)) => return Some(Err(e)),
            None => return None,
        };

        let commit = match self.repo.find_commit(rev) {
            Ok(commit) => commit,
            Err(e) => return Some(Err(e))
        };

        let diff = match self.get_last_diff(&commit) {
            Ok(diff) => diff,
            Err(e) => return Some(Err(e))
        };

        fn file_cb(_: DiffDelta, _: f32) -> bool {
            true
        }

        let mut change = Change::default();

        change.message = commit.message().map(String::from);

        let res = diff.foreach(
            &mut file_cb,
            None,
            None,
            Some(&mut |delta, _, line| -> bool {
                if line.origin() == '+' || line.origin() == '-' {
                    change.delta = delta.status();
                    let payload = serde_json::from_slice::<Crate>(line.content());

                    if let Ok(payload) = payload {
                        change.payload = payload;
                    }
                }
                true
            })
        );

        match res {
            Err(e) => Some(Err(e)),
            _ => Some(Ok(change))
        }
    }

}

#[test]
fn test() {
    let index = CratesIndex::new("crates.io-index").unwrap();
    let iter = index.iter().unwrap();

    for i in iter {
        println!("{:?}", i);
    }
}