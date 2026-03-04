from __future__ import annotations

import os
import sqlite3
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path
from typing import TypedDict, cast


class Note(TypedDict):
    id: int
    content: str
    created_at: str
    tags: list[str]


class Stats(TypedDict):
    total_notes: int
    tag_distribution: dict[str, int]
    notes_per_day: dict[str, int]


def get_db_path() -> Path:
    configured = os.environ.get("QUICKNOTE_DB_PATH")
    if configured:
        return Path(configured)
    return Path.home() / ".quicknote.db"


def get_connection() -> sqlite3.Connection:
    db_path = get_db_path()
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    _ = conn.execute("PRAGMA foreign_keys = ON")
    _init_schema(conn)
    return conn


def _init_schema(conn: sqlite3.Connection) -> None:
    _ = conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS note_tags (
            note_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            PRIMARY KEY (note_id, tag_id),
            FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            content,
            content='notes',
            content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, content) VALUES (new.id, new.content);
        END;

        CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, content) VALUES ('delete', old.id, old.content);
        END;

        CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, content) VALUES ('delete', old.id, old.content);
            INSERT INTO notes_fts(rowid, content) VALUES (new.id, new.content);
        END;
        """
    )


def add_note(content: str) -> int:
    created_at = datetime.now(UTC).isoformat(timespec="seconds")
    with get_connection() as conn:
        cursor = conn.execute(
            "INSERT INTO notes (content, created_at) VALUES (?, ?)",
            (content, created_at),
        )
        if cursor.lastrowid is None:
            raise RuntimeError("Failed to insert note")
        return int(cursor.lastrowid)


def add_tag(note_id: int, tag_name: str) -> bool:
    with get_connection() as conn:
        exists = cast(
            sqlite3.Row | None,
            conn.execute("SELECT 1 FROM notes WHERE id = ?", (note_id,)).fetchone(),
        )
        if exists is None:
            return False

        _ = conn.execute(
            "INSERT INTO tags (name) VALUES (?) ON CONFLICT(name) DO NOTHING",
            (tag_name,),
        )
        tag_row = cast(
            sqlite3.Row,
            conn.execute("SELECT id FROM tags WHERE name = ?", (tag_name,)).fetchone(),
        )
        _ = conn.execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
            (note_id, int(cast(int, tag_row["id"]))),
        )
    return True


def _rows_to_notes(rows: list[sqlite3.Row], conn: sqlite3.Connection) -> list[Note]:
    note_ids = [int(cast(int, row["id"])) for row in rows]
    tags_by_note: dict[int, list[str]] = {note_id: [] for note_id in note_ids}

    if note_ids:
        placeholders = ", ".join("?" for _ in note_ids)
        tag_rows = cast(
            list[sqlite3.Row],
            conn.execute(
                f"""
                SELECT nt.note_id, t.name
                FROM note_tags nt
                JOIN tags t ON t.id = nt.tag_id
                WHERE nt.note_id IN ({placeholders})
                ORDER BY t.name ASC
                """,
                note_ids,
            ).fetchall(),
        )
        for tag_row in tag_rows:
            note_id = int(cast(int, tag_row["note_id"]))
            tag_name = str(cast(str, tag_row["name"]))
            tags_by_note[note_id].append(tag_name)

    return [
        {
            "id": int(cast(int, row["id"])),
            "content": str(cast(str, row["content"])),
            "created_at": str(cast(str, row["created_at"])),
            "tags": tags_by_note[int(cast(int, row["id"]))],
        }
        for row in rows
    ]


def list_notes(tag: str | None = None) -> list[Note]:
    with get_connection() as conn:
        if tag:
            rows = conn.execute(
                """
                SELECT n.id, n.content, n.created_at
                FROM notes n
                JOIN note_tags nt ON nt.note_id = n.id
                JOIN tags t ON t.id = nt.tag_id
                WHERE t.name = ?
                ORDER BY n.id DESC
                """,
                (tag,),
            ).fetchall()
        else:
            rows = conn.execute(
                "SELECT id, content, created_at FROM notes ORDER BY id DESC"
            ).fetchall()
        return _rows_to_notes(rows, conn)


def search_notes(keyword: str) -> list[Note]:
    terms = [part.strip() for part in keyword.split() if part.strip()]
    fts_query = " ".join(f"{term}*" for term in terms) if terms else keyword
    with get_connection() as conn:
        rows = conn.execute(
            """
            SELECT n.id, n.content, n.created_at
            FROM notes_fts f
            JOIN notes n ON n.id = f.rowid
            WHERE notes_fts MATCH ?
            ORDER BY n.id DESC
            """,
            (fts_query,),
        ).fetchall()
        return _rows_to_notes(rows, conn)


def export_notes() -> list[Note]:
    return list_notes(tag=None)


def compute_stats() -> Stats:
    with get_connection() as conn:
        total_row = cast(
            sqlite3.Row,
            conn.execute("SELECT COUNT(*) AS c FROM notes").fetchone(),
        )
        total_notes = int(cast(int, total_row["c"]))
        tag_rows = cast(
            list[sqlite3.Row],
            conn.execute(
                """
                SELECT t.name, COUNT(*) AS count
                FROM note_tags nt
                JOIN tags t ON t.id = nt.tag_id
                GROUP BY t.name
                ORDER BY count DESC, t.name ASC
                """
            ).fetchall(),
        )
        tag_distribution = {
            str(cast(str, row["name"])): int(cast(int, row["count"]))
            for row in tag_rows
        }

        day_rows = cast(
            list[sqlite3.Row],
            conn.execute("SELECT created_at FROM notes").fetchall(),
        )
        histogram = Counter(str(cast(str, row["created_at"]))[:10] for row in day_rows)
        notes_per_day = dict(sorted(histogram.items()))

    return {
        "total_notes": total_notes,
        "tag_distribution": tag_distribution,
        "notes_per_day": notes_per_day,
    }
