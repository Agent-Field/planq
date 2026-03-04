# pyright: reportMissingImports=false, reportUnknownVariableType=false, reportUnknownMemberType=false
from fibonacci_api.api import create_app


def test_health_endpoint_returns_ok():
    app = create_app()
    client = app.test_client()

    response = client.get("/health")

    assert response.status_code == 200
    assert response.get_json() == {"status": "ok"}


def test_fib_endpoint_returns_value():
    app = create_app()
    client = app.test_client()

    response = client.get("/fib/7")

    assert response.status_code == 200
    assert response.get_json() == {"n": 7, "value": 13}


def test_fib_range_endpoint_returns_values():
    app = create_app()
    client = app.test_client()

    response = client.get("/fib/range/4/6")

    assert response.status_code == 200
    assert response.get_json() == {
        "start": 4,
        "end": 6,
        "values": [3, 5, 8],
    }


def test_fib_endpoint_rejects_value_above_limit():
    app = create_app()
    client = app.test_client()

    response = client.get("/fib/1001")

    assert response.status_code == 400
    assert "error" in response.get_json()


def test_fib_range_rejects_descending_bounds():
    app = create_app()
    client = app.test_client()

    response = client.get("/fib/range/8/5")

    assert response.status_code == 400
    assert "error" in response.get_json()


def test_fib_range_rejects_value_above_limit():
    app = create_app()
    client = app.test_client()

    response = client.get("/fib/range/1/1001")

    assert response.status_code == 400
    assert "error" in response.get_json()
