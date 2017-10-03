extern crate git2;

use git2::Repository;
use git2::*;

use std::path::Path;

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
    payload: String
}

impl Default for Change {
    fn default() -> Change {
        Change { delta: Delta::Modified, payload: String::new() }
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
    fn get_last_diff(&self, rev: Oid) -> Result<Diff, Error> {
        let commit = self.repo.find_commit(rev)?;
        let tree = self.repo.find_tree(commit.tree_id())?;

        let parent = commit.parent(0)?;

        let parent_tree = self.repo.find_tree(parent.tree_id())?;
        self.repo.diff_tree_to_tree(Some(&tree), Some(&parent_tree), None)
    }
}

impl<'a> Iterator for IndexIter<'a> {
    type Item = Result<Change, Error>;

    fn next(&mut self) -> Option<Result<Change, Error>> {
        let rev = match self.revwalk.next() {
            Some(Ok(r)) => r,
            Some(Err(e)) => return Some(Err(e)),
            None => return None
        };

        let diff = self.get_last_diff(rev).unwrap();

        fn file_cb(_: DiffDelta, _: f32) -> bool {
            true
        }

        let mut change = Change::default();

        let res = diff.foreach(
            &mut file_cb,
            None,
            None,
            Some(&mut |delta, _, line| -> bool {
                change.delta = delta.status();
                change.payload.push_str(std::str::from_utf8(line.content()).unwrap());
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

    for i in iter.take(5) {
        println!("{:?}", i);
    }
}