use anyhow::anyhow;
use std::{cmp::Ordering, collections::HashSet, fs::{self, File, OpenOptions}, io::{BufReader, Read, Seek, SeekFrom}, path::{Path, PathBuf}};

use crate::shared::stores::ObjectStore;

pub struct PackStore {
    base_path: PathBuf,
    pack_name: String,
    primary_file: PathBuf,
    index_file: PathBuf,
}

impl PackStore {
    pub fn new<P: AsRef<Path>>(base_path: P, pack_name: &str) -> Result<Self, anyhow::Error> {
        let base_path = base_path.as_ref();
        if !base_path.is_dir() {
            return Err(anyhow!("base path is not a directory"));
        }
        let primary_file = base_path.join(pack_name).with_added_extension("pack");
        if !primary_file.is_file() {
            return Err(anyhow!("pack file does not exist"));
        }
        let index_file = base_path.join(pack_name).with_added_extension("idx");
        if !index_file.is_file()
        {
            return Err(anyhow!("index file does not exist"));
        }
        Ok(PackStore {
            base_path: base_path.to_path_buf(),
            pack_name: pack_name.to_string(),
            primary_file,
            index_file,
        })
    }

    pub fn find_packs<P: AsRef<Path>>(base_path: P) -> Result<Vec<Self>, anyhow::Error> {
        let base_path = base_path.as_ref();
        if !base_path.is_dir() {
            return Err(anyhow!("base path is not a directory"));
        }
        let mut pack_names = HashSet::<String>::new();
        for dir_entry in fs::read_dir(base_path)? {
            let dir_entry = dir_entry?;
            let file_type = dir_entry.file_type()?;
            if file_type.is_file() {
                if let Some(candidate_pack) = Path::new(&dir_entry.file_name()).file_prefix() {
                    let candidate_pack = candidate_pack.to_string_lossy().to_string();
                    if candidate_pack.starts_with("pack-") && candidate_pack.chars().skip(5).all(|x| x.is_ascii_hexdigit()) {
                        pack_names.insert(candidate_pack);
                    }
                }
            }
        }
        Ok(pack_names.iter().map(|x| Self::new(base_path, x)).collect::<Result<Vec<Self>, anyhow::Error>>()?)
    }

    fn check_index_version<R>(index_file: &mut BufReader<R>) -> Result<bool, anyhow::Error> where R: Seek, R: Read {
        index_file.rewind()?;
        let mut buf = [0u8; 8];
        index_file.read_exact(&mut buf)?;
        Ok(buf == [255u8, 116, 79, 99, 0, 0, 0, 2])
    }

    fn get_index_offset_range<R>(index_file: &mut BufReader<R>, partial_object_id: &str) -> Result<PackIndexOffsetRange, anyhow::Error> where R: Seek, R: Read {
        if partial_object_id.len() < 2 || partial_object_id.chars().any(|c| !c.is_ascii_hexdigit()) {
            return Err(anyhow!("object ID is invalid or insufficient"))
        }
        let object_id_start = hex::decode(&partial_object_id[..2])?[0];
        let start_idx = if object_id_start == 0 {
            0u32
        } else {
            Self::get_index_offset(index_file, object_id_start - 1)?
        };
        let end_idx = Self::get_index_offset(index_file, object_id_start)?;
        Ok(PackIndexOffsetRange { start_idx, end_idx })
    }

    fn get_index_offset<R>(index_file: &mut BufReader<R>, object_id_start: u8) -> Result<u32, anyhow::Error> where R: Seek, R: Read {
        index_file.seek(std::io::SeekFrom::Start(u64::from(object_id_start) * 4 + 8))?;
        let mut buf = [0u8; 4];
        index_file.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    fn get_pack_item_count<R>(index_file: &mut BufReader<R>) -> Result<u32, anyhow::Error> where R: Read, R: Seek {
        index_file.seek(SeekFrom::Start(1028))?;
        let mut buf = [0u8; 4];
        index_file.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    fn search_index_objects<R>(index_file: &mut BufReader<R>, partial_object_id: &str) -> Result<Vec<PackIndexObject>, anyhow::Error> where R: Seek, R: Read {
        let mut results = Vec::<PackIndexObject>::new();
        let starting_range = Self::get_index_offset_range(index_file, partial_object_id)?;
        println!("Starting range is {starting_range:?}");
        if starting_range.is_empty() {
            return Ok(results)
        }
        let first_candidate = Self::find_index_object(index_file, partial_object_id, &starting_range)?;
        let Some(first_candidate) = first_candidate else {
            return Ok(results);
        };
        if partial_object_id.len() == 20 { // in other words, it's not partial
            return Ok(results);
        }
        let idx_in_range = first_candidate.idx;
        results.push(first_candidate);
        let mut idx_target = idx_in_range;
        loop {
            if idx_target == 0 {
                break;
            }
            idx_target -= 1;
            let obj_at_target = Self::get_index_object_id_at_pos(index_file, idx_target)?;
            if obj_at_target.starts_with(partial_object_id) {
                results.push(PackIndexObject { object_id: obj_at_target, idx: idx_target });
            } else {
                break;
            }
        }
        let top_limit = Self::get_pack_item_count(index_file)?;
        idx_target = idx_in_range;
        loop {
            idx_target += 1;
            if idx_target >= top_limit {
                break;
            }
            let obj_at_target = Self::get_index_object_id_at_pos(index_file, idx_target)?;
            if obj_at_target.starts_with(partial_object_id) {
                results.push(PackIndexObject { object_id: obj_at_target, idx: idx_target });
            } else {
                break;
            }
        }
        Ok(results)
    }

    fn find_index_object<R>(index_file: &mut BufReader<R>, partial_object_id: &str, obj_range: &PackIndexOffsetRange) -> Result<Option<PackIndexObject>, anyhow::Error> where R: Seek, R: Read {
        if obj_range.is_empty() {
            return Ok(None);
        }
        if obj_range.size() == 1 {
            let obj_id = Self::get_index_object_id_at_pos(index_file, obj_range.start_idx)?;
            println!("Range 1 compare: checking {obj_id}");
            if obj_id.starts_with(partial_object_id) {
                return Ok(Some(PackIndexObject { object_id: obj_id, idx: obj_range.start_idx }))
            } else {
                return Ok(None)
            }
        }
        let mid = obj_range.mid();
        let obj_id = Self::get_index_object_id_at_pos(index_file, mid)?;
        println!("Checking pos {mid} - {obj_id}");
        if obj_id.starts_with(partial_object_id) {
            return Ok(Some(PackIndexObject { object_id: obj_id, idx: mid }));
        }
        let recurse_range = match partial_object_id.cmp(&obj_id) {
            Ordering::Less => PackIndexOffsetRange { start_idx: obj_range.start_idx, end_idx: mid },
            Ordering::Greater => PackIndexOffsetRange { start_idx: mid + 1, end_idx: obj_range.end_idx },
            Ordering::Equal => {
                return Ok(Some(PackIndexObject { object_id: obj_id, idx: mid })); // here to satisfy the compiler; we've already ruled this out
            }
        };
        Self::find_index_object(index_file, partial_object_id, &recurse_range)
    }

    fn index_object_idx_to_id_offset(item_idx: u32) -> u64 {
        u64::from(item_idx) * 20 + 1032
    }

    fn get_index_object_id_at_pos<R>(index_file: &mut BufReader<R>, idx: u32) -> Result<String, anyhow::Error> where R: Seek, R: Read {
        index_file.seek(SeekFrom::Start(Self::index_object_idx_to_id_offset(idx)))?;
        let mut buf = [0u8; 20];
        index_file.read_exact(&mut buf)?;
        Ok(hex::encode(buf))
    }

    fn open_index_file(&self) -> Result<BufReader<File>, anyhow::Error> {
        let file = OpenOptions::new().read(true).open(&self.index_file)?;
        Ok(BufReader::new(file))
    }
}

impl ObjectStore for PackStore {
    fn _new_with_config(config: &std::collections::HashMap<String, String>) -> Result<Self, anyhow::Error>
    where
        Self: Sized {
        if !config.contains_key("base_path") {
            Err(anyhow!("base_path not set"))
        } else if !config.contains_key("pack_name") {
            Err(anyhow!("pack_name not set"))
        } else {
            PackStore::new(&config["base_path"], &config["pack_name"])
        }
    }

    fn create(&self) -> Result<(), anyhow::Error> {
        Err(anyhow!("pack creation not yet supported"))
    }

    fn _is_writeable(&self) -> bool {
        false
    }

    fn search_objects(&self, partial_object_id: &str) -> Result<Vec<String>, anyhow::Error> {
        println!("Searching for {partial_object_id} in {}", self.pack_name);
        let mut reader = self.open_index_file()?;
        if !Self::check_index_version(&mut reader)? {
            return Err(anyhow!("pack index file format not recognised"));
        }
        let found_objects = Self::search_index_objects(&mut reader, partial_object_id)?;
        println!("{} objects found in {}", found_objects.len(), self.pack_name);
        Ok(found_objects.into_iter().map(|x| x.object_id).collect())
    }

    fn has_object(&self, object_id: &str) -> Result<bool, anyhow::Error> {
        let mut reader = self.open_index_file()?;
        if !Self::check_index_version(&mut reader)? {
            return Err(anyhow!("pack index file format not recognised"));
        }
        let found_objects = Self::search_index_objects(&mut reader, object_id)?;
        Ok(!found_objects.is_empty())
    }

    fn read_object(&self, object_id: &str) -> Result<Option<crate::shared::objects::RawObject>, anyhow::Error> {
        todo!()
    }

    fn write_raw_object(&self, _obj: &crate::shared::objects::RawObject) -> Result<String, anyhow::Error> {
        Err(anyhow!("writing to packs not implemented"))
    }
}

#[derive(Debug)]
struct PackIndexOffsetRange {
    start_idx: u32,
    end_idx: u32,
}

impl PackIndexOffsetRange {
    fn is_empty(&self) -> bool {
        self.start_idx == self.end_idx
    }

    fn size(&self) -> u32 {
        self.end_idx - self.start_idx
    }

    fn mid(&self) -> u32 {
        self.start_idx + self.size() / 2
    }
}

struct PackIndexObject {
    object_id: String,
    idx: u32,
}
