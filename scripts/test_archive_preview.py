import ctypes
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

READER = Path(__file__).resolve().parents[1] / 'src/preview/archive_reader.py'


class ArchivePreviewTests(unittest.TestCase):
    def listing(self, source, directory):
        output = directory / 'listing.json'
        before = hashlib.sha256(source.read_bytes()).digest()
        result = subprocess.run([sys.executable, str(READER), str(source), str(output)], capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(hashlib.sha256(source.read_bytes()).digest(), before)
        return json.loads(output.read_text())

    def test_zip_lists_names_sizes_directories_and_links_without_extracting(self):
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            source = root / 'sample.zip'
            with zipfile.ZipFile(source, 'w') as archive:
                archive.writestr('Pasta/', '')
                archive.writestr('Pasta/ação.txt', 'hello')
                archive.writestr('../outside.txt', 'never extract')
                entry = zipfile.ZipInfo('link')
                entry.create_system = 3
                entry.external_attr = 0o120777 << 16
                archive.writestr(entry, '/etc/passwd')
            listing = self.listing(source, root)
            entries = {item['path']: item for item in listing['entries']}
            self.assertTrue(entries['Pasta/']['directory'])
            self.assertEqual(entries['Pasta/ação.txt']['size'], 5)
            self.assertTrue(entries['link']['link'])
            self.assertIn('../outside.txt', entries)
            self.assertFalse(listing['truncated'])
            self.assertEqual(set(root.iterdir()), {source, root / 'listing.json'})

    def test_tar_and_compressed_variants_list_the_same_headers_without_following_links(self):
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            for suffix, mode in [('.tar', 'w'), ('.tar.gz', 'w:gz'), ('.tar.bz2', 'w:bz2'), ('.tar.xz', 'w:xz')]:
                source = root / ('sample' + suffix)
                with tarfile.open(source, mode) as archive:
                    entry = tarfile.TarInfo('folder/note.txt')
                    entry.size = 5
                    archive.addfile(entry, io.BytesIO(b'hello'))
                    entry = tarfile.TarInfo('link')
                    entry.type = tarfile.SYMTYPE
                    entry.linkname = '/etc/passwd'
                    archive.addfile(entry)
                listing = self.listing(source, root)
                self.assertEqual(listing['entries'][0]['path'], 'folder/note.txt')
                self.assertEqual(listing['entries'][0]['size'], 5)
                self.assertTrue(listing['entries'][1]['link'])
                self.assertFalse((root / 'folder').exists())

    def test_single_compressed_stream_lists_its_contained_file(self):
        import gzip
        import bz2
        import lzma
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            for suffix, compress in [('.gz', gzip.compress), ('.bz2', bz2.compress), ('.xz', lzma.compress)]:
                source = root / ('note.txt' + suffix)
                source.write_bytes(compress(b'hello'))
                listing = self.listing(source, root)
                self.assertEqual(len(listing['entries']), 1)
                self.assertEqual(listing['entries'][0]['path'], 'note.txt')
                self.assertFalse(listing['entries'][0]['directory'])
                self.assertFalse(listing['truncated'])

    def test_listing_is_bounded_and_empty_zip_is_valid(self):
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            source = root / 'many.zip'
            with zipfile.ZipFile(source, 'w') as archive:
                for index in range(5005):
                    archive.writestr(f'{index}.txt', '')
            listing = self.listing(source, root)
            self.assertEqual(len(listing['entries']), 5000)
            self.assertTrue(listing['truncated'])
            source = root / 'empty.zip'
            with zipfile.ZipFile(source, 'w'):
                pass
            self.assertEqual(self.listing(source, root), {'entries': [], 'truncated': False})

    def test_corrupt_archives_report_short_errors_without_tracebacks(self):
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            for suffix in ['.zip', '.gz', '.7z']:
                source = root / ('broken' + suffix)
                source.write_bytes(b'invalid archive')
                result = subprocess.run([sys.executable, str(READER), str(source), str(root / 'output')], capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stderr, b'LSE_ARCHIVE_READ_FAILED')

    def test_7zip_headers_are_supported(self):
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            source = root / 'sample.7z'
            lib = ctypes.CDLL('libarchive.so.13')
            def fn(name, result, *args):
                f = getattr(lib, name)
                f.restype, f.argtypes = result, args
                return f
            ptr, integer, string = ctypes.c_void_p, ctypes.c_int, ctypes.c_char_p
            writer = fn('archive_write_new', ptr)()
            entry = fn('archive_entry_new', ptr)()
            try:
                self.assertEqual(fn('archive_write_set_format_7zip', integer, ptr)(writer), 0)
                self.assertEqual(fn('archive_write_open_filename', integer, ptr, string)(writer, bytes(source)), 0)
                fn('archive_entry_set_pathname', None, ptr, string)(entry, b'note.txt')
                fn('archive_entry_set_size', None, ptr, ctypes.c_int64)(entry, 5)
                fn('archive_entry_set_filetype', None, ptr, ctypes.c_uint)(entry, 0o100000)
                fn('archive_entry_set_perm', None, ptr, integer)(entry, 0o644)
                self.assertEqual(fn('archive_write_header', integer, ptr, ptr)(writer, entry), 0)
                self.assertEqual(fn('archive_write_data', ctypes.c_ssize_t, ptr, string, ctypes.c_size_t)(writer, b'hello', 5), 5)
                self.assertEqual(fn('archive_write_close', integer, ptr)(writer), 0)
            finally:
                fn('archive_entry_free', None, ptr)(entry)
                fn('archive_write_free', integer, ptr)(writer)
            listing = self.listing(source, root)
            self.assertEqual(listing['entries'][0]['path'], 'note.txt')
            self.assertEqual(listing['entries'][0]['size'], 5)


if __name__ == '__main__':
    unittest.main()
