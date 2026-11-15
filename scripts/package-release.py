#!/usr/bin/env python3
"""Build and verify binary archives with offline documentation (stdlib only)."""

import argparse
import html
import io
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import tarfile
import tempfile
from urllib.parse import quote, unquote, urlsplit
import zipfile


ROOT = Path(__file__).resolve().parent.parent
ROOT_FILES = ("README.md", "SPEC.md", "RELEASE.md", "TESTING.md", "LICENSE")
TARGETS = {
    "x86_64-unknown-linux-gnu": ("gb", "tar.gz"),
    "x86_64-apple-darwin": ("gb", "tar.gz"),
    "aarch64-apple-darwin": ("gb", "tar.gz"),
    "x86_64-pc-windows-msvc": ("gb.exe", "zip"),
}
REPOSITORY = "https://github.com/Masahiro-Obuchi/gitbucket-cli-rs"
LINK = re.compile(
    r"(?P<before>!?\[[^\]]*\]\()"
    r"(?P<url><[^>\n]+>|[^()\s]+)"
    r"(?P<after>(?:\s+\"[^\"\n]*\")?\))"
)
TOKEN = re.compile(r"`+[^`]*`+|" + LINK.pattern)


def map_links(text, transform):
    """Handle the inline Markdown links used by the project, outside code."""
    result, prose = [], []
    fence = None

    def replace(match):
        if match["url"] is None:
            return match[0]
        target = transform(match["url"].strip("<>"))
        if match["url"].startswith("<"):
            target = f"<{target}>"
        return match["before"] + target + match["after"]

    def flush():
        # Labels in the existing guides wrap across lines. Keep each prose
        # block together, while never interpreting a fenced example as a link.
        result.append(TOKEN.sub(replace, "".join(prose)))
        prose.clear()

    for line in text.splitlines(keepends=True):
        marker = re.match(r"^ {0,3}(`{3,}|~{3,})", line)
        if marker:
            flush()
            if fence is None:
                fence = marker[1]
            elif marker[1][0] == fence[0] and len(marker[1]) >= len(fence):
                fence = None
            result.append(line)
        elif fence:
            result.append(line)
        else:
            # A link label may itself contain inline code. Match complete links
            # before their contents, while leaving standalone code untouched.
            prose.append(line)
    flush()
    return "".join(result)


def anchors(text):
    prose = map_links(text, lambda url: url)
    # Omit fenced examples before inspecting headings.
    prose = re.sub(r"(?ms)^ {0,3}(`{3,}|~{3,})[^\n]*\n.*?^ {0,3}\1\s*$", "", prose)
    result, counts = set(), {}
    for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", prose, re.M):
        heading = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", heading)
        slug = re.sub(r"[^\w\s-]", "", html.unescape(heading).lower()).replace(" ", "-")
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        result.add(slug + (f"-{count}" if count else ""))
    return result


def regular_file(root, name):
    path = root / name
    # Reject symlinks in both files and ancestors, including a symlinked docs/.
    for parent in (path, *path.parents):
        if parent == root:
            break
        if parent.is_symlink():
            raise ValueError(f"symlink in public input: {name}")
    if not stat.S_ISREG(path.stat().st_mode):
        raise ValueError(f"not a regular public file: {name}")
    return path


def documentation(root, tag):
    names = set(ROOT_FILES)
    if (root / "docs").is_symlink():
        raise ValueError("symlink in public input: docs")
    for path in (root / "docs").rglob("*.md"):
        parts = path.relative_to(root).parts
        if any(p.startswith(".") or p.endswith(".md") for p in parts[:-1]):
            continue
        if not parts[-1].startswith(".") and path.is_file():
            names.add(path.relative_to(root).as_posix())
    files = {name: regular_file(root, name).read_bytes() for name in sorted(names)}

    def rewrite(current, url):
        parts = urlsplit(url)
        if parts.scheme or parts.netloc:
            return url
        decoded = unquote(parts.path)
        if "\\" in decoded or decoded.startswith("/") or parts.query:
            raise ValueError(f"unsupported local link in {current}: {url}")
        path = (
            (root / PurePosixPath(current).parent / decoded).resolve()
            if decoded else root / current
        )
        if not path.is_relative_to(root):
            raise ValueError(f"link escapes source tree in {current}: {url}")
        name = path.relative_to(root).as_posix()
        if name not in names:
            # Only source evidence may leave the bundle; never link local notes.
            if name != "Cargo.toml" and not name.startswith(("src/", "tests/", "scripts/")):
                raise ValueError(f"unapproved source link in {current}: {url}")
            if any(part.startswith(".") for part in PurePosixPath(name).parts):
                raise ValueError(f"hidden source link in {current}: {url}")
        regular_file(root, name)
        if parts.fragment and name.endswith(".md"):
            if unquote(parts.fragment) not in anchors(path.read_text(encoding="utf-8")):
                raise ValueError(f"missing anchor in {current}: {url}")
        if name in names:
            return url
        online = f"{REPOSITORY}/blob/{quote(tag, safe='')}/{quote(name, safe='/')}"
        return online + (f"#{parts.fragment}" if parts.fragment else "")

    for name in files:
        if name.endswith(".md"):
            text = files[name].decode("utf-8")
            files[name] = map_links(text, lambda url: rewrite(name, url)).encode("utf-8")
    return files


def verify_archive(archive, expected, binary_name, smoke_test):
    # Read only regular, explicitly expected members into a fresh directory.
    # Avoid extractall and its platform/version-dependent path/link semantics.
    with tempfile.TemporaryDirectory(prefix="gb-release-verify-") as temp:
        # macOS /var and Windows temporary paths can have filesystem aliases.
        # Compare both sides in the same canonical path representation.
        extracted = Path(temp).resolve()
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as package:
                members = package.infolist()
                # A byte-identical member may still be a symlink or directory
                # to an unzip tool. Windows ZIPs may omit Unix file-type bits.
                if any(
                    member.is_dir()
                    or member.external_attr & 0x10  # DOS directory attribute.
                    or stat.S_IFMT(member.external_attr >> 16) not in (0, stat.S_IFREG)
                    for member in members
                ):
                    raise ValueError("archive contains a non-regular member")
                names = [member.filename for member in members]
                actual = {member.filename: package.read(member) for member in members}
        else:
            with tarfile.open(archive, "r:gz") as package:
                members = package.getmembers()
                if any(not member.isfile() for member in members):
                    raise ValueError("archive contains a non-regular member")
                names = [member.name for member in members]
                actual = {member.name: package.extractfile(member).read() for member in members}
                executable = package.getmember(binary_name)
                if executable.mode & 0o111 != 0o111:
                    raise ValueError("archive binary is not executable")
        if len(names) != len(set(names)) or actual != expected:
            raise ValueError("archive contents differ from selected public inputs")
        for name, content in actual.items():
            path = extracted / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
        # Check links against the extracted tree, independently of source files.
        for name in actual:
            if not name.endswith(".md"):
                continue
            def check(url):
                parts = urlsplit(url)
                if not parts.scheme and not parts.netloc:
                    target = (
                        (extracted / PurePosixPath(name).parent / unquote(parts.path)).resolve()
                        if parts.path else extracted / name
                    )
                    if not target.is_relative_to(extracted) or not target.is_file():
                        raise ValueError(f"broken extracted link in {name}: {url}")
                    if parts.fragment:
                        headings = anchors(target.read_text(encoding="utf-8"))
                        if unquote(parts.fragment) not in headings:
                            raise ValueError(f"broken extracted anchor in {name}: {url}")
                return url
            map_links((extracted / name).read_text(encoding="utf-8"), check)
        if smoke_test:
            binary = extracted / binary_name
            binary.chmod(0o755)
            subprocess.run(
                [str(binary), "--help"], cwd=extracted, check=True,
                stdout=subprocess.PIPE, timeout=30,
            )


def package_release(root, binary, tag, target, output, smoke_test=False):
    root = root.resolve()
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", tag):
        raise ValueError("release tag must be a v-prefixed version")
    if target not in TARGETS:
        raise ValueError(f"unsupported release target: {target}")
    binary_name, extension = TARGETS[target]
    files = documentation(root, tag)
    if not binary.is_file() or binary.is_symlink():
        raise ValueError("binary must be a regular file")
    files[binary_name] = binary.read_bytes()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"gb-{tag}-{target}.{extension}"
    with tempfile.TemporaryDirectory(prefix=".gb-release-", dir=output) as temp:
        candidate = Path(temp) / archive.name
        if extension == "zip":
            with zipfile.ZipFile(candidate, "w", compression=zipfile.ZIP_DEFLATED) as package:
                for name, content in sorted(files.items()):
                    package.writestr(name, content)
        else:
            with tarfile.open(candidate, "w:gz") as package:
                for name, content in sorted(files.items()):
                    member = tarfile.TarInfo(name)
                    member.size = len(content)
                    member.mode = 0o755 if name == binary_name else 0o644
                    package.addfile(member, io.BytesIO(content))
        verify_archive(candidate, files, binary_name, smoke_test)
        os.replace(candidate, archive)
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/dist")
    parser.add_argument(
        "--smoke-test", action="store_true",
        help="run --help from the extracted native binary",
    )
    args = parser.parse_args()
    try:
        archive = package_release(
            ROOT, args.binary, args.tag, args.target, args.output_dir, args.smoke_test,
        )
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")
    print(f"Verified {archive}")


if __name__ == "__main__":
    main()
