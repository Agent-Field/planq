import json
from datetime import datetime, timezone
from pathlib import Path

from click.testing import CliRunner

from quicknote.cli import cli


def test_add_command_inserts_note(tmp_path: Path) -> None:
    db_path = tmp_path / "notes.db"
    runner = CliRunner()

    result = runner.invoke(cli, ["add", "my note"], env={"QUICKNOTE_DB": str(db_path)})

    assert result.exit_code == 0
    assert "Added note" in result.output


def test_list_command_shows_newest_first(tmp_path: Path) -> None:
    db_path = tmp_path / "notes.db"
    runner = CliRunner()
    env = {"QUICKNOTE_DB": str(db_path)}

    runner.invoke(cli, ["add", "first"], env=env)
    runner.invoke(cli, ["add", "second"], env=env)
    result = runner.invoke(cli, ["list"], env=env)

    assert result.exit_code == 0
    lines = [line for line in result.output.splitlines() if line.strip()]
    assert "second" in lines[0]
    assert "first" in lines[1]


def test_search_command_matches_note_content(tmp_path: Path) -> None:
    db_path = tmp_path / "notes.db"
    runner = CliRunner()
    env = {"QUICKNOTE_DB": str(db_path)}

    runner.invoke(cli, ["add", "buy milk"], env=env)
    runner.invoke(cli, ["add", "prepare sprint notes"], env=env)
    result = runner.invoke(cli, ["search", "sprint"], env=env)

    assert result.exit_code == 0
    assert "prepare sprint notes" in result.output
    assert "buy milk" not in result.output


def test_tag_command_adds_tag_to_note(tmp_path: Path) -> None:
    db_path = tmp_path / "notes.db"
    runner = CliRunner()
    env = {"QUICKNOTE_DB": str(db_path)}

    runner.invoke(cli, ["add", "finish report"], env=env)
    result = runner.invoke(cli, ["tag", "1", "work"], env=env)

    assert result.exit_code == 0
    assert "Tagged note 1 with work" in result.output


def test_list_command_filters_by_tag(tmp_path: Path) -> None:
    db_path = tmp_path / "notes.db"
    runner = CliRunner()
    env = {"QUICKNOTE_DB": str(db_path)}

    runner.invoke(cli, ["add", "deploy checklist"], env=env)
    runner.invoke(cli, ["add", "buy snacks"], env=env)
    runner.invoke(cli, ["tag", "1", "work"], env=env)
    result = runner.invoke(cli, ["list", "--tag", "work"], env=env)

    assert result.exit_code == 0
    assert "deploy checklist" in result.output
    assert "buy snacks" not in result.output


def test_export_command_outputs_json(tmp_path: Path) -> None:
    db_path = tmp_path / "notes.db"
    runner = CliRunner()
    env = {"QUICKNOTE_DB": str(db_path)}

    runner.invoke(cli, ["add", "first exportable note"], env=env)
    runner.invoke(cli, ["add", "second exportable note"], env=env)
    result = runner.invoke(cli, ["export", "--format", "json"], env=env)

    assert result.exit_code == 0
    payload = json.loads(result.output)
    assert len(payload) == 2
    assert payload[0]["content"] == "second exportable note"


def test_stats_command_reports_counts_tags_and_histogram(tmp_path: Path) -> None:
    db_path = tmp_path / "notes.db"
    runner = CliRunner()
    env = {"QUICKNOTE_DB": str(db_path)}

    runner.invoke(cli, ["add", "task alpha"], env=env)
    runner.invoke(cli, ["add", "task beta"], env=env)
    runner.invoke(cli, ["tag", "1", "work"], env=env)
    runner.invoke(cli, ["tag", "2", "personal"], env=env)
    result = runner.invoke(cli, ["stats"], env=env)

    assert result.exit_code == 0
    assert "Total notes: 2" in result.output
    assert "work: 1" in result.output
    assert "personal: 1" in result.output
    today = datetime.now(timezone.utc).date().isoformat()
    assert f"{today}: 2" in result.output
