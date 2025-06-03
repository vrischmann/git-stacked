use git2::{BranchType, ErrorCode, Oid, Repository};
use std::collections::{BTreeMap, HashMap, HashSet};

// Constants for coloring and mainline branches
const MAINLINE_BRANCH_NAMES_ARRAY: [&str; 5] = ["main", "master", "develop", "dev", "local-dev"];

const RED_START: &str = "\x1B[91m"; // Bright Red
const COLOR_RESET: &str = "\x1B[0m";
const DETACHED_PREFIX_TEXT: &str = "(detached)";

#[derive(Debug, onlyerror::Error)]
enum Error {
    #[error("git2 error: {0}")]
    Git2(#[from] git2::Error),

    #[error("repository is bare")]
    RepositoryIsBare,
}

#[derive(Debug, Clone)]
struct BranchInfo {
    name: String,
    oid: Oid,
}

struct ParentOfMap(HashMap<String, String>);

// Represents the parent-child relationships between branches.
// The key is the parent branch name, the values is a vector of child branch names.
struct ChildrenMap(BTreeMap<String, Vec<String>>); // BTreeMap for sorted keys

// Prints the ASCII tree structure in children_map recursively.
fn print_ascii_tree_recursive(
    parent_branch_name: &str,
    children_map: &ChildrenMap,
    current_prefix: &str,
) {
    if let Some(children_names) = children_map.0.get(parent_branch_name) {
        let num_children = children_names.len();
        for (i, child_name) in children_names.iter().enumerate() {
            let is_last_child = i == num_children - 1;
            let connector = if is_last_child {
                "└── "
            } else {
                "├── "
            };
            println!("{}{}{}", current_prefix, connector, child_name);

            let prefix_for_grandchildren = format!(
                "{}{}",
                current_prefix,
                if is_last_child { "    " } else { "│   " }
            );
            print_ascii_tree_recursive(child_name, children_map, &prefix_for_grandchildren);
        }
    }
}

// Retrieves all local branches in the repository and returns their names and OIDs.
fn get_branches(repo: &Repository) -> Result<Vec<BranchInfo>, Error> {
    let mut branches: Vec<BranchInfo> = Vec::new();
    let mut branch_iter = repo.branches(Some(BranchType::Local))?;

    while let Some(branch_result) = branch_iter.next() {
        let (branch, _) = branch_result?;

        if let (Some(name_ref), Some(target_oid)) = (branch.name()?, branch.get().target()) {
            branches.push(BranchInfo {
                name: name_ref.to_string(),
                oid: target_oid,
            });
        } else if let Ok(name_bytes) = branch.name_bytes() {
            eprintln!(
                "Warning: Branch name could not be processed or is not valid UTF-8: {:?}",
                String::from_utf8_lossy(name_bytes)
            );
        }
    }

    Ok(branches)
}

// Determines the parent-child relationships between branches based on their OIDs.
fn get_parent_of_relationships(
    repo: &Repository,
    branches: &Vec<BranchInfo>,
) -> Result<ParentOfMap, Error> {
    let mut parent_of = ParentOfMap(HashMap::new());

    for child_branch_info in branches {
        let child_name = &child_branch_info.name;
        let child_oid = child_branch_info.oid;

        let mut current_best_parent_name: Option<String> = None;
        let mut current_best_parent_oid: Option<Oid> = None;

        for potential_parent_info in branches {
            let potential_parent_name = &potential_parent_info.name;
            let potential_parent_oid = potential_parent_info.oid;

            if child_name == potential_parent_name || potential_parent_oid == child_oid {
                continue;
            }

            match repo.merge_base(potential_parent_oid, child_oid) {
                Ok(base_oid) if base_oid == potential_parent_oid => {
                    // potential_parent is an ancestor
                    if current_best_parent_name.is_none() {
                        current_best_parent_name = Some(potential_parent_name.clone());
                        current_best_parent_oid = Some(potential_parent_oid);
                    } else if let Some(cbp_oid) = current_best_parent_oid {
                        if cbp_oid != potential_parent_oid {
                            // Ensure we are looking at a different commit
                            match repo.merge_base(cbp_oid, potential_parent_oid) {
                                Ok(base_between_parents_oid)
                                    if base_between_parents_oid == cbp_oid =>
                                {
                                    // cbp_oid is an ancestor of potential_parent_oid,
                                    // meaning potential_parent is more specific/descendant.
                                    current_best_parent_name = Some(potential_parent_name.clone());
                                    current_best_parent_oid = Some(potential_parent_oid);
                                }
                                Err(e) if e.code() == ErrorCode::NotFound => { /* No common base, not ordered */
                                }
                                Err(e) => return Err(Error::Git2(e)),
                                _ => {}
                            }
                        }
                    }
                }
                Err(e) if e.code() == ErrorCode::NotFound => { /* No common base */ }
                Err(e) => return Err(Error::Git2(e)),
                _ => {} // Not an ancestor
            }
        }
        if let Some(p_name) = current_best_parent_name {
            parent_of.0.insert(child_name.clone(), p_name);
        }
    }

    Ok(parent_of)
}

struct ChildrenAndRoots {
    children_map: ChildrenMap,
    roots: Vec<String>,
}

// Builds the children_map and identifies root branches based on parent-child relationships.
fn build_children_and_roots(
    branches: &Vec<BranchInfo>,
    parent_of: &ParentOfMap,
) -> Result<ChildrenAndRoots, Error> {
    let mut children_map = ChildrenMap(BTreeMap::new());

    let mut all_branch_names: HashSet<String> = HashSet::new();
    for bi in branches {
        all_branch_names.insert(bi.name.clone());
    }

    let mut children_with_parents: HashSet<String> = HashSet::new();
    for (child, parent) in &parent_of.0 {
        children_map
            .0
            .entry(parent.clone())
            .or_default()
            .push(child.clone());
        children_with_parents.insert(child.clone());
    }

    // Sort children within each parent's list for deterministic output
    for children in children_map.0.values_mut() {
        children.sort();
    }

    let mut roots: Vec<String> = all_branch_names
        .difference(&children_with_parents)
        .cloned()
        .collect();
    roots.sort(); // Sort roots for deterministic output

    Ok(ChildrenAndRoots {
        children_map,
        roots,
    })
}

// Prints the branch tree structure based on the branches, parent-child relationships, and roots.
fn print_tree(
    branches: &Vec<BranchInfo>,
    parent_of: &ParentOfMap,
    children_map: &ChildrenMap,
    roots: &Vec<String>,
) -> Result<(), Error> {
    let mainline_branch_names: HashSet<&str> =
        MAINLINE_BRANCH_NAMES_ARRAY.iter().cloned().collect();

    if roots.is_empty() && !branches.is_empty() {
        if !&parent_of.0.is_empty() {
            // Structure exists but no clear roots (e.g. cycle, though unlikely)
            eprintln!(
                "Warning: Could not determine clear root(s) for branch tree. Check for unusual branch structures."
            );
            for bi in branches {
                // Fallback: print all branches flatly
                println!("{}", bi.name);
            }
        } else {
            // No parents found, all branches are effectively roots
            for bi in branches {
                let display_name = if mainline_branch_names.contains(bi.name.as_str()) {
                    bi.name.clone()
                } else {
                    format!(
                        "{}{}{} {}",
                        RED_START, DETACHED_PREFIX_TEXT, COLOR_RESET, bi.name
                    )
                };
                println!("{}", display_name);
                // children_map for this branch would be empty or not exist
                print_ascii_tree_recursive(&bi.name, &children_map, "");
            }
        }
        return Ok(());
    }

    for root_branch_name in roots {
        let display_name = if mainline_branch_names.contains(root_branch_name.as_str()) {
            root_branch_name.clone()
        } else {
            format!(
                "{}{}{} {}",
                RED_START, DETACHED_PREFIX_TEXT, COLOR_RESET, root_branch_name
            )
        };
        println!("{}", display_name);
        print_ascii_tree_recursive(root_branch_name, &children_map, "");
    }

    Ok(())
}

fn do_it() -> Result<(), Error> {
    let repo_path = Repository::discover(".")?
        .workdir()
        .ok_or(Error::RepositoryIsBare)?
        .to_path_buf();
    let repo = Repository::open(repo_path)?;

    // 1. Get local branches info (name and OID)
    let mut branches = get_branches(&repo)?;

    // Sort branch names for deterministic processing
    branches.sort_by(|a, b| a.name.cmp(&b.name));

    if branches.is_empty() {
        return Ok(());
    }

    // 2. Determine parent_of relationships
    let parent_of = get_parent_of_relationships(&repo, &branches)?;

    // 3. Build children_map (sorted by key for consistent iteration order) and identify roots
    let ChildrenAndRoots {
        children_map,
        roots,
    } = build_children_and_roots(&branches, &parent_of)?;

    // 4. Handle edge cases for printing & actual printing
    print_tree(&branches, &parent_of, &children_map, &roots)?;

    Ok(())
}

fn main() {
    do_it().unwrap()
}
