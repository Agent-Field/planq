# pyright: reportMissingImports=false, reportUnknownVariableType=false, reportUnknownMemberType=false, reportUnknownParameterType=false, reportUntypedFunctionDecorator=false, reportUnusedFunction=false
from flask import Flask, jsonify

from .fib import fibonacci_range, nth_fibonacci


def create_app() -> Flask:
    app = Flask(__name__)

    @app.get("/health")
    def health() -> tuple[dict[str, str], int]:
        return {"status": "ok"}, 200

    @app.get("/fib/<int:n>")
    def fib_value(n: int):
        try:
            value = nth_fibonacci(n)
        except ValueError as error:
            return jsonify({"error": str(error)}), 400

        return jsonify({"n": n, "value": value}), 200

    @app.get("/fib/range/<int:start>/<int:end>")
    def fib_values(start: int, end: int):
        try:
            values = fibonacci_range(start, end)
        except ValueError as error:
            return jsonify({"error": str(error)}), 400

        return jsonify({"start": start, "end": end, "values": values}), 200

    return app
