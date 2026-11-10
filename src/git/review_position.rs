//! Derive old/new coordinates from exact saved trees, never the working tree.
use super::{snapshot, GitCommandExt};
use crate::domain::{pull_request::PullRequestSnapshot, review_write::*};
use crate::error::Result;
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};

const MAX_INLINE_PATCH_BYTES: u64 = 32 * 1024 * 1024;

pub(crate) struct VerifiedLine {
    pub position: InlinePosition,
    pub diff: String,
}

fn git(args: &[&str]) -> Result<Vec<u8>> {
    let output = snapshot::command()
        .env_remove("GIT_DIFF_OPTS")
        .env_remove("GIT_LITERAL_PATHSPECS")
        .env_remove("GIT_GLOB_PATHSPECS")
        .env_remove("GIT_NOGLOB_PATHSPECS")
        .env_remove("GIT_ICASE_PATHSPECS")
        .args(args)
        .bounded_output()?;
    if !output.status.success() {
        return Err(invalid("Git could not verify the saved review diff."));
    }
    Ok(output.stdout)
}

pub(crate) fn verify(
    saved: &PullRequestSnapshot,
    path: &str,
    line: u64,
    side: Side,
) -> Result<VerifiedLine> {
    validate_path(path)?;
    if line == 0 || line > i32::MAX as u64 {
        return Err(invalid("Review line must be between 1 and 2147483647."));
    }
    let raw = git(&[
        "--literal-pathspecs",
        "diff",
        "--raw",
        "-z",
        "--no-abbrev",
        "--no-ext-diff",
        "--no-textconv",
        "--no-relative",
        "--find-renames=50%",
        saved.base.as_str(),
        saved.head.as_str(),
        "--",
    ])?;
    let paths = changed_file(&raw, path)?;
    let literal_paths: Vec<_> = paths
        .iter()
        .map(|path| format!(":(top,literal){path}"))
        .collect();
    let mut args = vec![
        "-c",
        "diff.algorithm=myers",
        "-c",
        "diff.suppressBlankEmpty=false",
        "diff",
        "--unified=3",
        "--inter-hunk-context=0",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--no-relative",
        "--find-renames=50%",
        saved.base.as_str(),
        saved.head.as_str(),
        "--",
    ];
    args.extend(literal_paths.iter().map(String::as_str));
    let mut file = tempfile::tempfile()?;
    let output = snapshot::command()
        // GIT_DIFF_OPTS takes precedence over --unified=3. The parser also
        // needs the normal space prefix on every unchanged row, even blanks.
        .env_remove("GIT_DIFF_OPTS")
        .env_remove("GIT_LITERAL_PATHSPECS")
        .env_remove("GIT_GLOB_PATHSPECS")
        .env_remove("GIT_NOGLOB_PATHSPECS")
        .env_remove("GIT_ICASE_PATHSPECS")
        .args(args)
        .bounded_output_to_file(file.try_clone()?)?;
    if !output.status.success() {
        return Err(invalid("Git could not produce the saved inline diff."));
    }
    let patch = read_patch(&mut file)?;
    if patch.contains('\0') {
        return Err(invalid("Binary review targets are unsupported."));
    }
    line_in_patch(&patch, path, line, side)
}

fn read_patch(file: &mut std::fs::File) -> Result<String> {
    if file.metadata()?.len() > MAX_INLINE_PATCH_BYTES {
        return Err(invalid("The selected text diff exceeds the 32 MiB inline validation limit. No comment was posted."));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(MAX_INLINE_PATCH_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INLINE_PATCH_BYTES {
        return Err(invalid(
            "Inline diff grew beyond the 32 MiB validation limit.",
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        invalid("Review diff must be UTF-8 text; binary/non-UTF-8 files are unsupported.")
    })
}

fn changed_file(raw: &[u8], selected: &str) -> Result<Vec<String>> {
    let fields: Vec<_> = raw.split(|b| *b == 0).collect();
    let mut index = 0;
    let mut found = None;
    while index + 1 < fields.len() {
        let header =
            std::str::from_utf8(fields[index]).map_err(|_| invalid("Invalid Git diff header."))?;
        let header: Vec<_> = header.split_whitespace().collect();
        if header.len() != 5 || !header[0].starts_with(':') {
            return Err(invalid("Invalid Git raw diff."));
        }
        let mode_old = &header[0][1..];
        let mode_new = header[1];
        let status = header[4].as_bytes()[0];
        let first = fields[index + 1];
        index += 2;
        let second = if matches!(status, b'R' | b'C') {
            let second = fields
                .get(index)
                .ok_or_else(|| invalid("Missing rename target."))?;
            index += 1;
            Some(*second)
        } else {
            None
        };
        let name = second.unwrap_or(first);
        if name == selected.as_bytes() {
            if found.is_some()
                || ![mode_old, mode_new]
                    .iter()
                    .all(|mode| matches!(*mode, "000000" | "100644" | "100755"))
            {
                return Err(invalid(
                    "Ambiguous path, symlink or submodule review target.",
                ));
            }
            let names = match second {
                Some(second) => vec![first, second],
                None => vec![first],
            };
            found = Some(
                names
                    .into_iter()
                    .map(|name| {
                        String::from_utf8(name.to_vec())
                            .map_err(|_| invalid("Non-UTF-8 rename source cannot be verified."))
                    })
                    .collect::<Result<Vec<_>>>()?,
            );
        }
    }
    found.ok_or_else(|| invalid("File is not in the saved PR diff. For a rename use the destination path; for a deletion use the old path."))
}

fn range_start(range: &str) -> Result<u64> {
    range
        .get(1..)
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| invalid("Invalid Git hunk coordinates."))
}

fn line_in_patch(patch: &str, path: &str, wanted: u64, side: Side) -> Result<VerifiedLine> {
    if patch
        .lines()
        .filter(|l| l.starts_with("diff --git "))
        .count()
        != 1
    {
        return Err(invalid("Expected exactly one text file diff."));
    }
    let (mut old, mut new) = (0, 0);
    let mut in_hunk = false;
    let mut context: Vec<Value> = Vec::new();
    for row in patch.lines() {
        if row.starts_with("@@ ") {
            let ranges: Vec<_> = row.split_whitespace().collect();
            old = range_start(ranges.get(1).ok_or_else(|| invalid("Missing old range."))?)?;
            new = range_start(ranges.get(2).ok_or_else(|| invalid("Missing new range."))?)?;
            context.clear();
            in_hunk = true;
            continue;
        }
        if !in_hunk || row.starts_with('\\') {
            continue;
        }
        let (o, n, kind) = match row.as_bytes().first() {
            Some(b' ') => (Some(old), Some(new), "equal"),
            Some(b'-') => (Some(old), None, "delete"),
            Some(b'+') => (None, Some(new), "insert"),
            _ => return Err(invalid("Unrecognized Git diff row.")),
        };
        let mut snippet = json!({"type":kind,"text":&row[1..]});
        if let Some(o) = o {
            snippet["oldLine"] = json!(o.to_string());
            old += 1;
        }
        if let Some(n) = n {
            snippet["newLine"] = json!(n.to_string());
            new += 1;
        }
        context.push(snippet);
        if context.len() > 4 {
            context.remove(0);
        }
        if match side {
            Side::Old => o,
            Side::New => n,
        } == Some(wanted)
        {
            return Ok(VerifiedLine {
                position: InlinePosition {
                    path: path.into(),
                    old_line: o,
                    new_line: n,
                },
                diff: serde_json::to_string(&context)?,
            });
        }
    }
    Err(invalid("Line is not present on the selected side of a text diff hunk (3 context lines). No comment was posted."))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_patch_is_rejected_from_metadata_before_loading_it_into_memory() {
        let mut file = tempfile::tempfile().unwrap();
        file.set_len(MAX_INLINE_PATCH_BYTES + 1).unwrap();
        assert!(read_patch(&mut file).is_err());
    }
    #[test]
    fn maps_insert_delete_and_context_without_treating_patch_offsets_as_line_numbers() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -8,2 +8,2 @@\n same\n-old\n+new\n";
        let old = line_in_patch(patch, "x", 9, Side::Old).unwrap();
        assert_eq!(
            (old.position.old_line, old.position.new_line),
            (Some(9), None)
        );
        let new = line_in_patch(patch, "x", 9, Side::New).unwrap();
        assert_eq!(
            (new.position.old_line, new.position.new_line),
            (None, Some(9))
        );
        let context = line_in_patch(patch, "x", 8, Side::New).unwrap();
        assert_eq!(
            (context.position.old_line, context.position.new_line),
            (Some(8), Some(8))
        );
        assert!(line_in_patch(patch, "x", 1, Side::New).is_err());
    }
}
