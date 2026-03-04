from __future__ import annotations

import json
import os
from pathlib import Path
from typing import cast

import click

from quicknote.storage import QuickNoteStore


def resolve_db_path() -> Path:
    env_db_path = os.getenv("QUICKNOTE_DB")
    if env_db_path:
        return Path(env_db_path)
    return Path.cwd() / "quicknote.db"


def store_from_context(ctx: click.Context) -> QuickNoteStore:
    ctx_obj = cast(dict[str, object], ctx.obj)
    return cast(QuickNoteStore, ctx_obj["store"])


@click.group()
@click.pass_context
def cli(ctx: click.Context) -> None:
    _ = ctx.ensure_object(dict)
    ctx.obj["store"] = QuickNoteStore(resolve_db_path())


@cli.command("add")
@click.argument("content")
@click.pass_context
def add_command(ctx: click.Context, content: str) -> None:
    store = store_from_context(ctx)
    note_id = store.add_note(content)
    click.echo(f"Added note {note_id}")


@cli.command("list")
@click.option("--tag", "tag_filter", type=str)
@click.pass_context
def list_command(ctx: click.Context, tag_filter: str | None) -> None:
    store = store_from_context(ctx)
    notes = store.list_notes(tag=tag_filter)
    for note in notes:
        click.echo(f"[{note['id']}] {note['created_at']} {note['content']}")


@cli.command("search")
@click.argument("query")
@click.pass_context
def search_command(ctx: click.Context, query: str) -> None:
    store = store_from_context(ctx)
    notes = store.search_notes(query)
    for note in notes:
        click.echo(f"[{note['id']}] {note['created_at']} {note['content']}")


@cli.command("tag")
@click.argument("note_id", type=int)
@click.argument("tag")
@click.pass_context
def tag_command(ctx: click.Context, note_id: int, tag: str) -> None:
    store = store_from_context(ctx)
    if not store.tag_note(note_id, tag):
        raise click.ClickException(f"Note {note_id} not found")
    click.echo(f"Tagged note {note_id} with {tag}")


@cli.command("export")
@click.option("--format", "output_format", type=click.Choice(["json"]), required=True)
@click.pass_context
def export_command(ctx: click.Context, output_format: str) -> None:
    store = store_from_context(ctx)
    if output_format == "json":
        click.echo(json.dumps(store.export_notes()))


@cli.command("stats")
@click.pass_context
def stats_command(ctx: click.Context) -> None:
    store = store_from_context(ctx)
    stats = store.stats()
    click.echo(f"Total notes: {stats['total_notes']}")
    click.echo("Tag distribution:")
    tag_distribution = cast(list[dict[str, object]], stats["tag_distribution"])
    for item in tag_distribution:
        click.echo(f"{item['tag']}: {item['count']}")
    click.echo("Notes per day:")
    notes_per_day = cast(list[dict[str, object]], stats["notes_per_day"])
    for item in notes_per_day:
        click.echo(f"{item['day']}: {item['count']}")
