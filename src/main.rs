use std::{
  collections::{BTreeMap, HashSet, VecDeque},
  path::{Path, PathBuf},
};

use bstr::ByteSlice;

use unicase::UniCase;

#[rustfmt::skip]
fn print_help() {
  println!("whist is a word-histogram sort of utility.");
  println!("--print-by-frequency     Will print the words by frequency.");
  println!("--case-sensitive         Will make searches case sensitive.");
}

fn main() {
  let args: Vec<String> = std::env::args().collect();
  if args.iter().any(|s| s.as_str() == "--help") {
    print_help();
    return;
  }
  let print_by_frequency =
    if args.iter().any(|s| s.as_str() == "--print-by-frequency") { true } else { false };
  let case_sensitive =
    if args.iter().any(|s| s.as_str() == "--case-sensitive") { true } else { false };

  const TEN_MEGABYTES: usize = 10 * 1024 * 1024;
  let mut buf = Vec::with_capacity(TEN_MEGABYTES);
  let mut intern: HashSet<&'static str> = HashSet::new();
  let mut word_counts: BTreeMap<UniCase<&'static str>, usize> = BTreeMap::new();
  let mut word_counts_cased: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut biggest_word: usize = 0;

  recursive_read_dir(".", |p| {
    match std::fs::File::open(&p) {
      Err(e) => eprintln!("Couldn't open {path}: {e}", path = p.display(), e = e),
      Ok(mut f) => match std::io::Read::read_to_end(&mut f, &mut buf) {
        Err(e) => eprintln!("Error while reading {path}: {e}", path = p.display(), e = e),
        Ok(_byte_count_read) => {
          for word in buf.words() {
            let interned_letters: &'static str = intern.get(word).copied().unwrap_or_else(|| {
              let leaked: &'static str = Box::leak(String::from(word).into_boxed_str());
              intern.insert(leaked);
              biggest_word = biggest_word.max(leaked.len());
              leaked
            });
            if case_sensitive {
              *word_counts_cased.entry(interned_letters).or_insert(0) += 1;
            } else {
              *word_counts.entry(UniCase::new(interned_letters)).or_insert(0) += 1;
            }
          }
        }
      },
    }
    buf.clear();
  });

  if print_by_frequency {
    use std::cmp::Ordering;
    let mut v: Vec<(&'static str, usize)> = if case_sensitive {
      word_counts_cased.into_iter().collect()
    } else {
      word_counts.into_iter().map(|(uc, c)| (uc.into_inner(), c)).collect()
    };
    v.sort_unstable_by(|(w1, c1), (w2, c2)| match c1.cmp(c2) {
      Ordering::Less => Ordering::Greater,
      Ordering::Greater => Ordering::Less,
      Ordering::Equal => w1.cmp(w2),
    });
    for (word, count) in v.iter() {
      println!(
        "{word:>biggest_word$}: {count}",
        word = word,
        count = count,
        biggest_word = biggest_word
      );
    }
  } else {
    for (word, count) in word_counts.iter() {
      println!(
        "{word:>biggest_word$}: {count}",
        word = word,
        count = count,
        biggest_word = biggest_word
      );
    }
  }
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
