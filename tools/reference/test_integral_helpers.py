"""Tests for integral comparison utilities that do not require PySCF or Rust."""

import numpy as np
import pytest
from integrals import metrics


def test_metrics_detect_failure_and_nonfinite():
    assert metrics([0.0, 1.0], [0.0, 1.01], [(0,), (1,)])["exceedances"] == 1
    with pytest.raises(ValueError, match="non-finite"):
        metrics([np.nan], [0.0], [(0,)])
