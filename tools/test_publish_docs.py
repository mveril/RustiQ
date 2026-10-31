import importlib.util
import json
from pathlib import Path

import pytest

spec = importlib.util.spec_from_file_location('publish_docs', Path(__file__).with_name('publish-docs.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def test_publication_lifecycle(tmp_path):
    book, site = tmp_path / 'book', tmp_path / 'site'
    book.mkdir()
    (book / 'index.html').write_text('first')
    (book / '404.html').write_text('<base href="./">')
    (book / '404.html').chmod(0o444)
    site.mkdir()
    (site / 'legacy.html').write_text('legacy')
    commit = 'a' * 40
    module.publish(book, site, commit)
    assert (site / 'dev/index.html').read_text() == 'first'
    assert not (site / 'stable').exists()
    module.publish(book, site, commit, 'v0.1.0-beta.1')
    snapshot = (site / 'v0.1.0-beta.1/index.html').read_bytes()
    (book / 'index.html').write_text('second')
    module.publish(book, site, 'b' * 40, 'v0.1.0-beta.2')
    module.publish(book, site, 'c' * 40, 'v0.1.0')
    assert (site / 'v0.1.0-beta.1/index.html').read_bytes() == snapshot
    assert (site / 'beta/index.html').read_text() == 'second'
    assert (site / 'stable/index.html').read_text() == 'second'
    assert (site / 'legacy.html').read_text() == 'legacy'
    (book / 'index.html').write_text('first')
    module.publish(book, site, commit, 'v0.1.0-beta.1')
    module.publish(book, site, commit, 'v0.1.0-beta.1')
    data = json.loads((site / 'versions.json').read_text())
    assert [v['version'] for v in data['versions']] == ['v0.1.0', 'v0.1.0-beta.2', 'v0.1.0-beta.1']
    assert data['latest_beta'] == 'v0.1.0-beta.2'
    assert data['latest_stable'] == 'v0.1.0'
    assert data['versions'][-1]['commit'] == commit
    with pytest.raises(ValueError, match='conflict'):
        module.publish(book, site, 'd' * 40, 'v0.1.0-beta.1')
    (book / 'index.html').write_text('changed')
    with pytest.raises(ValueError, match='conflict'):
        module.publish(book, site, commit, 'v0.1.0-beta.1')


@pytest.mark.parametrize('version', ['v01.0.0', 'v1.0.0-beta.01', '../dev', 'v1.0', 'v1.0.0+build'])
def test_invalid_version(version):
    with pytest.raises(ValueError):
        module.version_key(version)
