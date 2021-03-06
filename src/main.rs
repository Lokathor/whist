use std::{
  collections::{BTreeMap, HashSet, VecDeque},
  path::{Path, PathBuf},
};

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let print_by_frequency =
    if args.iter().any(|s| s.as_str() == "--print-by-frequency") { true } else { false };

  const TEN_MEGABYTES: usize = 10 * 1024 * 1024;
  let mut buf = Vec::with_capacity(TEN_MEGABYTES);
  let mut intern: HashSet<&'static str> = HashSet::new();
  let mut word_counts: BTreeMap<&'static str, usize> = BTreeMap::new();

  recursive_read_dir(".", |p| {
    match std::fs::File::open(&p) {
      Err(e) => eprintln!("Couldn't open {path}: {e}", path = p.display(), e = e),
      Ok(mut f) => match std::io::Read::read_to_end(&mut f, &mut buf) {
        Err(e) => eprintln!("Error while reading {path}: {e}", path = p.display(), e = e),
        Ok(_byte_count_read) => match core::str::from_utf8(&buf) {
          Err(_) => {
            eprintln!("Error: {path} is not utf8. TODO: support non-utf8.", path = p.display())
          }
          Ok(s) => {
            for term in StrBreaker::new(s) {
              match term {
                Term::Letters(letters) => {
                  let interned_letters: &'static str =
                    intern.get(letters).copied().unwrap_or_else(|| {
                      let leaked: &'static str = Box::leak(String::from(letters).into_boxed_str());
                      intern.insert(leaked);
                      leaked
                    });
                  *word_counts.entry(interned_letters).or_insert(0) += 1;
                }
                _ => (),
              }
            }
          }
        },
      },
    }
    buf.clear();
  });

  if print_by_frequency {
    use std::cmp::Ordering;
    let mut v: Vec<(&'static str, usize)> = word_counts.into_iter().collect();
    v.sort_unstable_by(|(w1, c1), (w2, c2)| match c1.cmp(c2) {
      Ordering::Less => Ordering::Greater,
      Ordering::Greater => Ordering::Less,
      Ordering::Equal => w1.cmp(w2),
    });
    for (word, count) in v.iter() {
      println!("{word}: {count}", word = word, count = count);
    }
  } else {
    for (word, count) in word_counts.iter() {
      println!("{word}: {count}", word = word, count = count);
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Term<'s> {
  Letters(&'s str),
  Symbols(&'s str),
}
struct StrBreaker<'s> {
  spare: &'s str,
}
impl<'s> StrBreaker<'s> {
  pub fn new(spare: &'s str) -> Self {
    Self { spare: spare.trim() }
  }

  pub fn is_kinda_letter(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
  }
}
impl<'s> Iterator for StrBreaker<'s> {
  type Item = Term<'s>;

  fn next(&mut self) -> Option<Term<'s>> {
    if self.spare.is_empty() {
      return None;
    }
    let c = self.spare.chars().nth(0).unwrap();
    if StrBreaker::is_kinda_letter(c) {
      if let Some((i, _)) =
        self.spare.char_indices().find(|(_, c)| !StrBreaker::is_kinda_letter(*c))
      {
        let out = Term::Letters(&self.spare[..i]);
        self.spare = &self.spare[i..];
        Some(out)
      } else {
        let out = Term::Letters(self.spare);
        self.spare = "";
        Some(out)
      }
    } else {
      if let Some((i, _)) = self.spare.char_indices().find(|(_, c)| StrBreaker::is_kinda_letter(*c))
      {
        let out = Term::Symbols(&self.spare[..i]);
        self.spare = &self.spare[i..];
        Some(out)
      } else {
        let out = Term::Symbols(self.spare);
        self.spare = "";
        Some(out)
      }
    }
  }
}

#[test]
fn test_str_breaker() {
  let mut sb = StrBreaker::new("_abc.words();");
  assert_eq!(sb.next(), Some(Term::Letters("_abc")));
  assert_eq!(sb.next(), Some(Term::Symbols(".")));
  assert_eq!(sb.next(), Some(Term::Letters("words")));
  assert_eq!(sb.next(), Some(Term::Symbols("();")));
  assert_eq!(sb.next(), None);
}

/// Recursively walks over the `path` given, which must be a directory.
///
/// Your `op` is passed a [`PathBuf`] for each file found.
pub fn recursive_read_dir(path: impl AsRef<Path>, mut op: impl FnMut(PathBuf)) {
  let path = path.as_ref();
  assert!(path.is_dir());
  // Note(Lokathor): Being *literally* recursive can blow out the stack for no
  // reason. Instead, we use a queue based system. Each loop pulls a dir out of
  // the queue and walks it.
  // * If we find a sub-directory that goes into the queue for later.
  // * Files get passed to the `op`
  // * Symlinks we check if they point to a Dir or File and act accordingly.
  //
  // REMINDER: if a symlink makes a loop on the file system then this will trap
  // us in an endless loop. That's the user's fault!
  let mut path_q = VecDeque::new();
  path_q.push_back(PathBuf::from(path));
  while let Some(path_buf) = path_q.pop_front() {
    match std::fs::read_dir(&path_buf) {
      Err(e) => eprintln!("Can't read_dir {path}: {e}", path = path_buf.display(), e = e),
      Ok(read_dir) => {
        for result_dir_entry in read_dir {
          match result_dir_entry {
            Err(e) => eprintln!("Error with dir entry: {e}", e = e),
            Ok(dir_entry) => match dir_entry.file_type() {
              Ok(ft) if ft.is_dir() => path_q.push_back(dir_entry.path()),
              Ok(ft) if ft.is_file() => op(dir_entry.path()),
              Ok(ft) if ft.is_symlink() => match dir_entry.metadata() {
                Ok(metadata) if metadata.is_dir() => path_q.push_back(dir_entry.path()),
                Ok(metadata) if metadata.is_file() => op(dir_entry.path()),
                Err(e) => eprintln!(
                  "Can't get metadata for symlink {path}: {e}",
                  path = dir_entry.path().display(),
                  e = e
                ),
                _ => eprintln!(
                  "Found symlink {path} but it's not a file or a directory.",
                  path = dir_entry.path().display()
                ),
              },
              Err(e) => eprintln!(
                "Can't get file type of {path}: {e}",
                path = dir_entry.path().display(),
                e = e
              ),
              _ => eprintln!(
                "Found dir_entry {path} but it's not a file, directory, or symlink.",
                path = dir_entry.path().display()
              ),
            },
          }
        }
      }
    }
  }
}
