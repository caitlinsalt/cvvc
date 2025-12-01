use anyhow::{anyhow, Context};
use flate2::{bufread::ZlibEncoder, read::ZlibDecoder, Compression};
use ini::Ini;
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{BufReader, Cursor, Read},
    path::{Path, PathBuf},
};

pub struct Repository {
    worktree: PathBuf,
    git_dir: PathBuf,
    conf: Ini,
}

impl Repository {
    pub fn new(worktree: &PathBuf, allow_invalid: bool) -> Result<Self, anyhow::Error> {
        let my_worktree = worktree.clone();
        let my_git_dir = worktree.join(Path::new(".git"));
        if !(allow_invalid || my_git_dir.is_dir()) {
            return Err(anyhow!("Not a git directory"));
        }
        let config_path = repo_path(&my_git_dir, Path::new("config"));
        let mut wrapped_config: Option<Ini> = None;
        if config_path.is_file() {
            let loaded_config = Ini::load_from_file(config_path);
            if loaded_config.is_err() {
                if !allow_invalid {
                    return Err(anyhow::Error::from(loaded_config.unwrap_err())
                        .context("Could not open configuration file"));
                }
            } else {
                wrapped_config = Some(loaded_config.unwrap());
            }
        } else if !allow_invalid {
            return Err(anyhow!("Configuration file missing"));
        }

        let config = wrapped_config.unwrap_or_else(|| default_config());

        if !allow_invalid {
            let core_section = match config.section(Some("core")) {
                Some(s) => s,
                None => {
                    return Err(anyhow!(
                        "Configuration file does not contain a [core] section"
                    ))
                }
            };
            let format_version_property = match core_section.get("repositoryformatversion") {
                Some(s) => s,
                None => {
                    return Err(anyhow!(
                        "Configuration file does not have the repository format version set"
                    ))
                }
            };
            let format_version = format_version_property
                .parse::<i32>()
                .context("repositoryformatversion is not an integer")?;
            if format_version != 0 {
                return Err(anyhow!("Unsupported repository version {format_version}"));
            }
        }

        Ok(Repository {
            worktree: my_worktree,
            git_dir: my_git_dir,
            conf: config,
        })
    }

    pub fn create(path: &PathBuf) -> Result<Self, anyhow::Error> {
        let repo = Repository::new(path, true)?;

        if repo.worktree.exists() {
            if !repo.worktree.is_dir() {
                return Err(anyhow!(format!(
                    "Path {} is not a directory",
                    repo.worktree.display()
                )));
            }
            if repo.git_dir.exists() {
                if !repo.git_dir.is_dir() {
                    return Err(anyhow!(format!(
                        "Path {} is not a directory",
                        repo.git_dir.display()
                    )));
                }
                let mut dir_contents = repo
                    .git_dir
                    .read_dir()
                    .context("Could not attempt to read contents of repository")?;
                if dir_contents.next().is_some() {
                    return Err(anyhow!("Repository directory is not empty"));
                }
            }
        } else {
            fs::create_dir_all(&repo.worktree)
                .context("Could not create all components of directory path")?;
        }

        repo.dir(Path::new("branches"), true)?;
        repo.dir(Path::new("objects"), true)?;
        repo.dir(&["refs", "tags"].iter().collect::<PathBuf>(), true)?;
        repo.dir(&["refs", "heads"].iter().collect::<PathBuf>(), true)?;

        fs::write(
            repo.file_unchecked(Path::new("description")),
            "Unnamed repository\n",
        )?;

        fs::write(
            repo.file_unchecked(Path::new("HEAD")),
            "ref: refs/heads/main\n",
        )?;

        repo.conf
            .write_to_file(repo.file_unchecked(Path::new("config")))?;

        Ok(repo)
    }

    pub fn _path(&self, path: &Path) -> PathBuf {
        repo_path(&self.git_dir, path)
    }

    pub fn file(&self, path: &Path, mkdir: bool) -> Result<Option<PathBuf>, anyhow::Error> {
        repo_file(&self.git_dir, path, mkdir)
    }

    pub fn file_unchecked(&self, path: &Path) -> PathBuf {
        self.file(path, false).unwrap().unwrap()
    }

    pub fn dir(&self, path: &Path, mkdir: bool) -> Result<Option<PathBuf>, anyhow::Error> {
        repo_dir(&self.git_dir, path, mkdir)
    }

    pub fn _dir_unchecked(&self, path: &Path) -> PathBuf {
        self.dir(path, false).unwrap().unwrap()
    }

    pub fn find_object<'a>(&'a self, name: &'a str) -> &'a str {
        name
    }

    pub fn object_read(&self, sha: &str) -> Result<Option<Box<impl GitObject>>, anyhow::Error> {
        let path = self.file_unchecked(
            &["objects", &sha[..2], &sha[2..]]
                .iter()
                .collect::<PathBuf>(),
        );
        if !path.is_file() {
            return Ok(None);
        }
        let file = fs::File::open(path)?;
        let mut decompressor = ZlibDecoder::new(file);
        let mut data: Vec<u8> = vec![];
        decompressor.read_to_end(&mut data)?;
        let type_end_index = data.iter().position(|&x| x == 0x20).ok_or(anyhow!(
            "Malformed object {sha}: end of object type code not found"
        ))?;
        let len_start_index = type_end_index + 1;
        let len_end_index = data
            .iter()
            .skip(len_start_index)
            .position(|&x| x == 0)
            .ok_or(anyhow!(
                "Malformed object {sha}: end of object length not found"
            ))?
            + len_start_index;
        let data_start_index = len_end_index + 1;
        let object_type = &data[..type_end_index];
        let object_len = std::str::from_utf8(&data[len_start_index..len_end_index])?
            .parse::<usize>()
            .context(format!(
                "Could not parse object length!  Object length string was {}",
                std::str::from_utf8(&data[len_start_index..len_end_index])?
            ))?;
        let actual_len = data.len() - data_start_index;
        if object_len != actual_len {
            return Err(anyhow!(
                "Malformed object {sha}: expected length {object_len}, actual length {actual_len}"
            ));
        }

        match object_type {
            b"blob" => Ok(Some(Box::new(Blob::deserialise(&data[data_start_index..])))),
            _ => Err(anyhow!(format!(
                "Unrecognised object type {}",
                std::str::from_utf8(object_type).unwrap_or("[mangled]")
            ))),
        }
    }
}

pub trait GitObject {
    type Implementation;
    fn object_type_code(&self) -> &'static [u8];
    fn serialise(&self) -> &Vec<u8>;
    fn deserialise(data: &[u8]) -> Self::Implementation
    where
        Self: Sized;
}

pub fn object_write(
    obj: &impl GitObject,
    repo: Option<&Repository>,
) -> Result<Option<String>, anyhow::Error> {
    let data = obj.serialise();
    let mut content = obj.object_type_code().to_vec();
    content.extend(b" ");
    content.extend(data.len().to_string().into_bytes());
    content.extend(b"\x00");
    content.extend(data);

    let mut hasher = Sha1::new();
    hasher.update(&content);
    let hash = hex::encode(hasher.finalize());

    if repo.is_some() {
        let the_repo = repo.unwrap();
        let path = the_repo.file(
            &["objects", &hash[0..2], &hash[2..]]
                .iter()
                .collect::<PathBuf>(),
            true,
        )?;
        if path.is_some() {
            let the_path = path.unwrap();
            if !the_path.exists() {
                let mut file = fs::File::create(the_path)?;
                let mut compressor =
                    ZlibEncoder::new(BufReader::new(Cursor::new(content)), Compression::best());
                std::io::copy(&mut compressor, &mut file)?;
            }
        }
    }

    Ok(Some(hash))
}

pub struct Blob {
    data: Vec<u8>,
}

impl Blob {
    pub fn new_from_read(source: &mut impl Read) -> Result<Self, anyhow::Error> {
        let mut buf: Vec<u8> = Vec::new();
        source
            .read_to_end(&mut buf)
            .context("Failed to read blob from source")?;
        Ok(Blob { data: buf })
    }
}

impl GitObject for Blob {
    type Implementation = Blob;

    fn object_type_code(&self) -> &'static [u8] {
        b"blob"
    }

    fn serialise(&self) -> &Vec<u8> {
        &self.data
    }

    fn deserialise(data: &[u8]) -> Self::Implementation
    where
        Self: Sized,
    {
        Blob {
            data: data.to_vec(),
        }
    }
}

fn repo_path(git_dir: &PathBuf, path: &Path) -> PathBuf {
    git_dir.join(path)
}

fn repo_file(
    git_dir: &PathBuf,
    path: &Path,
    mkdir: bool,
) -> Result<Option<PathBuf>, anyhow::Error> {
    let file_name = path.file_name();
    if file_name.is_none() {
        return Err(anyhow!("Path must not be a directory"));
    }
    let base_path = path.parent().unwrap_or(Path::new(""));
    let dir_path = repo_dir(git_dir, base_path, mkdir)?;
    Ok(match dir_path {
        Some(the_path) => Some(the_path.join(file_name.unwrap())),
        None => None,
    })
}

fn repo_dir(git_dir: &PathBuf, path: &Path, mkdir: bool) -> Result<Option<PathBuf>, anyhow::Error> {
    let path = repo_path(git_dir, path);
    check_and_create_dir(path, mkdir)
}

fn check_and_create_dir(path: PathBuf, mkdir: bool) -> Result<Option<PathBuf>, anyhow::Error> {
    if path.exists() {
        if path.is_dir() {
            return Ok(Some(path));
        }
        return Err(anyhow!("Path exists but is not a directory"));
    }
    if mkdir {
        fs::create_dir_all(&path).context("Could not create all components of directory path")?;
        return Ok(Some(path));
    }
    Ok(None)
}

fn default_config() -> Ini {
    let mut conf = Ini::new();
    conf.with_section(Some("core"))
        .set("repositoryformatversion", "0")
        .set("filemode", "false")
        .set("bare", "false");
    conf
}

pub fn repo_find(path: &Path) -> Result<Option<Repository>, anyhow::Error> {
    let path_buf = path.to_path_buf().canonicalize()?;
    if path_buf.join(Path::new(".git")).is_dir() {
        return Ok(Some(Repository::new(&path_buf, false)?));
    }
    match path_buf.parent() {
        Some(p) => repo_find(p),
        None => Ok(None),
    }
}
