"""Exercise the actual release packager against disposable source trees."""

import importlib.util
from pathlib import Path
import stat
import subprocess
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile

SCRIPT = Path(__file__).resolve().parents[1] / "package-release.py"
spec = importlib.util.spec_from_file_location("package_release", SCRIPT)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)

# Independent distribution contract: removing a guide or target in the
# packager must not silently remove the same expectation from these tests.
PUBLIC_ROOT = ("README.md", "SPEC.md", "RELEASE.md", "TESTING.md", "LICENSE")
ARCHIVES = (
    ("x86_64-unknown-linux-gnu", "gb", "tar.gz"),
    ("x86_64-apple-darwin", "gb", "tar.gz"),
    ("aarch64-apple-darwin", "gb", "tar.gz"),
    ("x86_64-pc-windows-msvc", "gb.exe", "zip"),
)


class ReleaseArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "source"
        self.root.mkdir()
        self.output = Path(self.temp.name) / "output"
        for name in PUBLIC_ROOT:
            self.write(name, f"# {name}\n")
        self.write(
            "README.md", "# Guide\n\n[Contract](docs/contract.md#details)\n[Spec](SPEC.md)\n",
        )
        self.write(
            "docs/contract.md", "# Contract\n\n## Details\n"
            "[Home](../README.md#guide)\n[Evidence](../tests/evidence.rs)\n",
        )
        self.write("tests/evidence.rs", "// public evidence\n")
        self.binary = Path(self.temp.name) / "compiled-binary"
        self.binary.write_bytes(b"synthetic executable bytes\n")

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content.encode("utf-8"))

    def package(self, target="x86_64-unknown-linux-gnu", tag="v1.2.3-rc.1+docs"):
        return release.package_release(self.root, self.binary, tag, target, self.output)

    def contents(self, archive):
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as package:
                return {name: package.read(name) for name in package.namelist()}
        with tarfile.open(archive) as package:
            return {m.name: package.extractfile(m).read() for m in package.getmembers()}

    def test_all_release_targets_retain_docs_and_use_tagged_source_links(self):
        for target, binary, extension in ARCHIVES:
            with self.subTest(target=target):
                archive = self.package(target)
                files = self.contents(archive)
                self.assertTrue(archive.name.endswith("." + extension))
                self.assertEqual(set(files), set(PUBLIC_ROOT) | {"docs/contract.md", binary})
                self.assertEqual(files[binary], self.binary.read_bytes())
                self.assertIn(b"(docs/contract.md#details)", files["README.md"])
                self.assertIn(b"(../README.md#guide)", files["docs/contract.md"])
                self.assertIn(
                    b"/blob/v1.2.3-rc.1%2Bdocs/tests/evidence.rs", files["docs/contract.md"],
                )
                self.assertNotIn(b"main/", files["docs/contract.md"])
        self.assertIn("../tests/evidence.rs", (self.root / "docs/contract.md").read_text())

    def test_only_public_documents_are_selected(self):
        for name in [
            "review/private.md", ".codex", ".tmp/runtime.env", "docs/.private.md",
            "docs/.cache/notes.md", "docs/local.md/nested.md", "docs/runtime.env",
            "docs/debug.txt", "unrelated/guide.md",
        ]:
            self.write(name, "synthetic local data\n")
        self.write("docs/nested/guide.md", "# More\n")
        files = self.contents(self.package())
        self.assertEqual(
            set(files), set(PUBLIC_ROOT) | {"gb", "docs/contract.md", "docs/nested/guide.md"},
        )

    def test_missing_file_and_anchor_stop_packaging(self):
        for url in ["docs/missing.md", "docs/contract.md#missing", "tests/missing.rs"]:
            with self.subTest(url=url):
                self.write("README.md", f"# Guide\n[Broken]({url})\n")
                with self.assertRaises((ValueError, OSError)):
                    self.package()
                self.assertFalse(self.output.exists())

    def test_local_notes_and_escaping_links_are_rejected(self):
        self.write("review/private.md", "# Private\n")
        self.write("tests/.private.rs", "// local\n")
        for url in ["review/private.md", "../outside.md", "tests/.private.rs", "/README.md"]:
            with self.subTest(url=url):
                self.write("README.md", f"[Bad]({url})\n")
                with self.assertRaises(ValueError):
                    self.package()

    def test_fenced_and_inline_examples_are_unchanged(self):
        examples = (
            "```md\n[Example](missing.md)\n```\n~~~md\n"
            "[Example](missing.md)\n~~~\n`[Example](missing.md)`\n"
        )
        self.write("README.md", "# Guide\n" + examples + "[Source `evidence`](tests/evidence.rs)\n")
        text = self.contents(self.package())["README.md"].decode()
        self.assertIn(examples, text)
        self.assertIn("[Source `evidence`](https://github.com/", text)

    def test_spaces_titles_and_duplicate_heading_anchors(self):
        self.write("docs/a guide.md", "# Same\n# Same\n")
        self.write("README.md", '# Guide\n[Spaces](<docs/a guide.md#same-1> "A guide")\n')
        files = self.contents(self.package())
        self.assertIn(b'(<docs/a guide.md#same-1> "A guide")', files["README.md"])

    def test_wrapped_link_labels_are_rewritten_and_checked(self):
        self.write("README.md", "# Guide\n[Source\n evidence](tests/evidence.rs)\n")
        files = self.contents(self.package())
        self.assertIn(b"[Source\n evidence](https://github.com/", files["README.md"])
        self.write("README.md", "[Missing\n document](docs/missing.md)\n")
        with self.assertRaises((OSError, ValueError)):
            self.package()

    def test_symlinked_document_cannot_copy_another_file(self):
        link = self.root / "docs/alias.md"
        try:
            link.symlink_to(self.root / "README.md")
        except OSError:
            self.skipTest("symlink creation unavailable")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.package()

    def test_verification_handles_a_symlinked_temporary_parent(self):
        actual = Path(self.temp.name) / "actual-temp"
        actual.mkdir()
        alias = Path(self.temp.name) / "temp-alias"
        try:
            alias.symlink_to(actual, target_is_directory=True)
        except OSError:
            self.skipTest("directory symlink creation unavailable")
        make_temp = tempfile.TemporaryDirectory

        def temporary_directory(*args, **kwargs):
            if kwargs.get("prefix") == "gb-release-verify-":
                kwargs["dir"] = alias
            return make_temp(*args, **kwargs)

        with mock.patch.object(release.tempfile, "TemporaryDirectory", temporary_directory):
            self.assertIn("docs/contract.md", self.contents(self.package()))

    def test_crlf_document_bytes_and_links_are_preserved(self):
        original = b"# Guide\r\n[Contract](docs/contract.md#details)\r\n`example`\r\n"
        (self.root / "README.md").write_bytes(original)
        self.assertEqual(self.contents(self.package())["README.md"], original)

    def test_invalid_tag_or_target_does_not_create_output(self):
        for tag in ["../outside", "v1.2", "v1.2.3/other", "v1.2.3\n"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                self.package(tag=tag)
        with self.assertRaises(ValueError):
            self.package(target="unknown")
        self.assertFalse(self.output.exists())

    def test_previous_binary_readme_license_layout_has_broken_navigation(self):
        archive = self.package("x86_64-pc-windows-msvc")
        old = {
            name: content for name, content in self.contents(archive).items()
            if name in ("gb.exe", "README.md", "LICENSE")
        }
        with zipfile.ZipFile(archive, "w") as package:
            for name, content in old.items():
                package.writestr(name, content)
        with self.assertRaisesRegex(ValueError, "broken extracted link"):
            release.verify_archive(archive, old, "gb.exe", False)

    def test_verifier_rejects_missing_docs_and_changed_binary(self):
        archive = self.package("x86_64-pc-windows-msvc")
        expected = self.contents(archive)
        for omitted in ["docs/contract.md", "gb.exe", None]:
            with zipfile.ZipFile(archive, "w") as package:
                for name, content in expected.items():
                    if name != omitted:
                        updated = b"changed" if omitted is None and name == "gb.exe" else content
                        package.writestr(name, updated)
            with self.assertRaisesRegex(ValueError, "contents differ"):
                release.verify_archive(archive, expected, "gb.exe", False)

    def test_verifier_rejects_zip_non_regular_member_attributes(self):
        archive = self.package("x86_64-pc-windows-msvc")
        expected = self.contents(archive)
        attributes = [
            (stat.S_IFLNK | 0o777) << 16,
            (stat.S_IFDIR | 0o755) << 16,
            (stat.S_IFIFO | 0o600) << 16,
            (0o600 << 16) | 0x10,  # DOS directory bit without a trailing slash.
        ]
        for changed in ["README.md", "gb.exe"]:
            for attribute in attributes:
                with self.subTest(member=changed, attribute=attribute):
                    with zipfile.ZipFile(archive, "w") as package:
                        for name, content in expected.items():
                            info = zipfile.ZipInfo(name)
                            info.create_system = 3
                            info.external_attr = (
                                attribute if name == changed
                                else (stat.S_IFREG | 0o644) << 16
                            )
                            package.writestr(info, content)
                    with self.assertRaisesRegex(ValueError, "non-regular member"):
                        release.verify_archive(archive, expected, "gb.exe", False)

    def test_verifier_checks_navigation_after_extracting(self):
        archive = self.package("x86_64-pc-windows-msvc")
        expected = self.contents(archive)
        expected["README.md"] = b"[Broken](absent.md)\n"
        with zipfile.ZipFile(archive, "w") as package:
            for name, content in expected.items():
                package.writestr(name, content)
        with self.assertRaisesRegex(ValueError, "broken extracted link"):
            release.verify_archive(archive, expected, "gb.exe", False)

    def test_smoke_test_failure_does_not_replace_an_existing_archive(self):
        archive = self.package()
        before = archive.read_bytes()
        with self.assertRaises((OSError, subprocess.SubprocessError)):
            release.package_release(
                self.root, self.binary, "v1.2.3-rc.1+docs", "x86_64-unknown-linux-gnu",
                self.output, smoke_test=True,
            )
        self.assertEqual(archive.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
