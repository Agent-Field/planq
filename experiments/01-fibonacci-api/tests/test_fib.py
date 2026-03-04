# pyright: reportMissingImports=false, reportUnknownVariableType=false, reportUnknownParameterType=false, reportMissingParameterType=false, reportUnknownArgumentType=false, reportUnusedCallResult=false
import pytest

from fibonacci_api.fib import fibonacci_range, nth_fibonacci


def test_nth_fibonacci_returns_expected_value():
    assert nth_fibonacci(10) == 55


def test_fibonacci_range_returns_expected_values():
    assert fibonacci_range(5, 8) == [5, 8, 13, 21]


@pytest.mark.parametrize("n", [0, -1, 1001])
def test_nth_fibonacci_rejects_out_of_bounds(n):
    with pytest.raises(ValueError):
        nth_fibonacci(n)


def test_fibonacci_range_rejects_descending_bounds():
    with pytest.raises(ValueError):
        fibonacci_range(9, 4)
