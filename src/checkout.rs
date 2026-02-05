use crate::shared::{objects::StoredObject, repo::Repository};
use anyhow::anyhow;
use std::{fs, path::Path};

pub fn checkout(obj_name: &str, dest: &str) -> Result<(), anyhow::Error> {
    let repo = Repository::find_cwd()?;
    match repo {
        Some(repo) => checkout_from_repo(&repo, obj_name, dest),
        None => Ok(()),
    }
}

fn checkout_from_repo(repo: &Repository, target_name: &str, dest: &str) -> Result<(), anyhow::Error> {
    let target_id = repo.find_object(target_name, None, true)?;
    let obj = repo.read_object(&target_id)?;
    let Some(obj) = obj else {
        return Err(anyhow!("Object {} not found", target_name));
    };
    let tree_obj = match obj {
        StoredObject::Tree(tree) => tree,
        StoredObject::Commit(commit) => {
            let tree_entry = commit.map().get("tree");
            let Some(tree_entry) = tree_entry else {
                return Err(anyhow!("Commit {} is missing a tree", target_id));
            };
            let Some(tree_entry) = tree_entry.first() else {
                return Err(anyhow!("Commit {} has an empty tree entry", target_id));
            };
            let Some(tree_obj) = repo.read_object(tree_entry)? else {
                return Err(anyhow!("Commit {} points to a non-existent tree", target_id));
            };
            let StoredObject::Tree(tree_obj) = tree_obj else {
                return Err(anyhow!(
                    "Commit {} points to a non-tree object as its tree",
                    target_id
                ));
            };
            tree_obj
        }
        _ => {
            return Err(anyhow!(
                "Object {} is not a commit-ish or tree-ish thing",
                target_id
            ));
        }
    };
    let path = Path::new(dest);
    if path.exists() {
        if !path.is_dir() {
            return Err(anyhow!("Path {} is not a directory", dest));
        }
        if !is_dir_empty(path)? {
            return Err(anyhow!("Path {} is not empty", dest));
        }
    } else {
        fs::create_dir_all(path)?;
    }

    let objects_checked_out = tree_obj.checkout(repo, path)?;
    let mut index = repo.read_index()?;
    index.remove_not_present(&objects_checked_out);
    repo.write_index(&index)?;

    if repo.is_branch_name(target_name)? {
        repo.update_head(target_name)
    } else {
        repo.update_head_detached(&target_id)?;
        println!("HEAD is detached at {target_id}");
        Ok(())
    }
}

fn is_dir_empty(dir: &Path) -> Result<bool, anyhow::Error> {
    let mut entries = fs::read_dir(dir)?;
    let first_entry = entries.next();
    Ok(first_entry.is_none())
}
