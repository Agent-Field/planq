MAX_N = 1000


def _validate_n(value: int) -> None:
    if value <= 0:
        raise ValueError("value must be positive")
    if value > MAX_N:
        raise ValueError(f"value must be <= {MAX_N}")


def nth_fibonacci(n: int) -> int:
    _validate_n(n)
    if n <= 2:
        return 1

    prev = 1
    curr = 1
    for _ in range(3, n + 1):
        prev, curr = curr, prev + curr
    return curr


def fibonacci_range(start: int, end: int) -> list[int]:
    _validate_n(start)
    _validate_n(end)
    if start > end:
        raise ValueError("start must be <= end")

    return [nth_fibonacci(index) for index in range(start, end + 1)]
