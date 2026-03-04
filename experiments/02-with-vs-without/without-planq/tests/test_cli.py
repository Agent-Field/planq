import json
from pathlib import Path
from typing import cast

from click.testing import CliRunner, Result

from quicknote.cli import cli


def run_cli(tmp_path: Path, args: list[str]) -> Result:
    runner = CliRunner()
    env = {"QUICKNOTE_DB_PATH": str(tmp_path / "quicknote.db")}
    return runner.invoke(cli, args, env=env)


def test_add_and_list_orders_newest_first(tmp_path: Path):
    add1 = run_cli(tmp_path, ["add", "first note"])
    assert add1.exit_code == 0
    assert "Added note 1" in add1.output

    add2 = run_cli(tmp_path, ["add", "second note"])
    assert add2.exit_code == 0
    assert "Added note 2" in add2.output

    listed = run_cli(tmp_path, ["list"])
    assert listed.exit_code == 0
    lines = [line for line in listed.output.strip().splitlines() if line]

    assert "second note" in lines[0]
    assert "first note" in lines[1]


def test_search_finds_matching_notes(tmp_path: Path):
    _ = run_cli(tmp_path, ["add", "write architecture doc"])
    _ = run_cli(tmp_path, ["add", "buy groceries"])

    result = run_cli(tmp_path, ["search", "arch"])
    assert result.exit_code == 0
    assert "architecture doc" in result.output
    assert "groceries" not in result.output


def test_tag_and_filter_by_tag(tmp_path: Path):
    _ = run_cli(tmp_path, ["add", "finish project spec"])
    _ = run_cli(tmp_path, ["add", "plan vacation"])

    tagged = run_cli(tmp_path, ["tag", "1", "work"])
    assert tagged.exit_code == 0
    assert "Tagged note 1 with 'work'" in tagged.output

    filtered = run_cli(tmp_path, ["list", "--tag", "work"])
    assert filtered.exit_code == 0
    assert "finish project spec" in filtered.output
    assert "plan vacation" not in filtered.output


def test_export_json_to_stdout(tmp_path: Path):
    _ = run_cli(tmp_path, ["add", "draft launch email"])
    _ = run_cli(tmp_path, ["tag", "1", "marketing"])

    exported = run_cli(tmp_path, ["export", "--format", "json"])
    assert exported.exit_code == 0

    payload = cast(list[dict[str, object]], json.loads(exported.output))
    assert isinstance(payload, list)
    assert len(payload) == 1
    assert payload[0]["content"] == "draft launch email"
    assert payload[0]["tags"] == ["marketing"]


def test_stats_reports_counts_distribution_and_histogram(tmp_path: Path):
    _ = run_cli(tmp_path, ["add", "note one"])
    _ = run_cli(tmp_path, ["add", "note two"])
    _ = run_cli(tmp_path, ["tag", "1", "work"])
    _ = run_cli(tmp_path, ["tag", "2", "work"])
    _ = run_cli(tmp_path, ["tag", "2", "ideas"])

    stats = run_cli(tmp_path, ["stats"])
    assert stats.exit_code == 0
    assert "Total notes: 2" in stats.output
    assert "work: 2" in stats.output
    assert "ideas: 1" in stats.output
    assert "Notes per day:" in stats.output
