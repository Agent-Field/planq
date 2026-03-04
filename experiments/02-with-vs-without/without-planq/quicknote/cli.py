from __future__ import annotations

import json

import click

from quicknote import db


def _render_notes(notes: list[db.Note]) -> None:
    if not notes:
        click.echo("No notes found.")
        return

    for note in notes:
        tags = ",".join(note["tags"])
        click.echo(f"{note['id']}\t{note['created_at']}\t{note['content']}\t[{tags}]")


@click.group()
def cli() -> None:
    pass


@cli.command("add")
@click.argument("content")
def add_command(content: str) -> None:
    note_id = db.add_note(content)
    click.echo(f"Added note {note_id}")


@cli.command("list")
@click.option("--tag", "tag_name", default=None, help="Filter notes by tag name.")
def list_command(tag_name: str | None) -> None:
    notes = db.list_notes(tag=tag_name)
    _render_notes(notes)


@cli.command("search")
@click.argument("keyword")
def search_command(keyword: str) -> None:
    notes = db.search_notes(keyword)
    _render_notes(notes)


@cli.command("tag")
@click.argument("note_id", type=int)
@click.argument("tag_name")
def tag_command(note_id: int, tag_name: str) -> None:
    updated = db.add_tag(note_id, tag_name)
    if not updated:
        raise click.ClickException(f"Note {note_id} does not exist.")
    click.echo(f"Tagged note {note_id} with '{tag_name}'")


@cli.command("export")
@click.option(
    "--format",
    "output_format",
    type=click.Choice(["json"], case_sensitive=False),
    default="json",
    show_default=True,
)
def export_command(output_format: str) -> None:
    if output_format.lower() != "json":
        raise click.ClickException("Only json export is supported.")
    notes = db.export_notes()
    click.echo(json.dumps(notes, indent=2))


@cli.command("stats")
def stats_command() -> None:
    stats = db.compute_stats()
    click.echo(f"Total notes: {stats['total_notes']}")
    click.echo("Tag distribution:")
    if stats["tag_distribution"]:
        for tag_name, count in stats["tag_distribution"].items():
            click.echo(f"- {tag_name}: {count}")
    else:
        click.echo("- (none)")

    click.echo("Notes per day:")
    if stats["notes_per_day"]:
        for day, count in stats["notes_per_day"].items():
            click.echo(f"- {day}: {count}")
    else:
        click.echo("- (none)")


if __name__ == "__main__":
    cli()
