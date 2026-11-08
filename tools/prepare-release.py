"""Compatibility entry point for read-only release validation.

Release tags are created by Promote release after merging into main.
"""

import runpy
from pathlib import Path

if __name__ == '__main__':
    runpy.run_path(str(Path(__file__).with_name('check-release.py')), run_name='__main__')
