from __future__ import annotations

import sqlite3
from collections.abc import Iterator
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import cast


class QuickNoteStore:
    def __init__(self, db_path: str | Path) -> None:
        self.db_path: Path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self._init_db()

    @contextmanager
    def connection(self) -> Iterator[sqlite3.Connection]:
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        try:
            yield conn
            conn.commit()
        finally:
            conn.close()

    def _init_db(self) -> None:
        with self.connection() as conn:
            _ = conn.executescript(
                """
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS notes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS note_tags (
                    note_id INTEGER NOT NULL,
                    tag TEXT NOT NULL,
                    PRIMARY KEY (note_id, tag),
                    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
                );

                CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts
                USING fts5(content, content='notes', content_rowid='id');

                CREATE TRIGGER IF NOT EXISTS notes_ai
                AFTER INSERT ON notes
                BEGIN
                    INSERT INTO notes_fts(rowid, content)
                    VALUES (new.id, new.content);
                END;

                CREATE TRIGGER IF NOT EXISTS notes_ad
                AFTER DELETE ON notes
                BEGIN
                    INSERT INTO notes_fts(notes_fts, rowid, content)
                    VALUES ('delete', old.id, old.content);
                END;

                CREATE TRIGGER IF NOT EXISTS notes_au
                AFTER UPDATE ON notes
                BEGIN
                    INSERT INTO notes_fts(notes_fts, rowid, content)
                    VALUES ('delete', old.id, old.content);
                    INSERT INTO notes_fts(rowid, content)
                    VALUES (new.id, new.content);
                END;
                """
            )

    def add_note(self, content: str) -> int:
        created_at = datetime.now(timezone.utc).isoformat()
        with self.connection() as conn:
            cursor = conn.execute(
                "INSERT INTO notes(content, created_at) VALUES(?, ?)",
                (content, created_at),
            )
            note_id = cursor.lastrowid
            if note_id is None:
                raise RuntimeError("Failed to insert note")
            return note_id

    def list_notes(self, tag: str | None = None) -> list[sqlite3.Row]:
        with self.connection() as conn:
            if tag is None:
                rows = conn.execute(
                    "SELECT id, content, created_at FROM notes ORDER BY id DESC"
                ).fetchall()
            else:
                rows = conn.execute(
                    """
                    SELECT n.id, n.content, n.created_at
                    FROM notes n
                    JOIN note_tags t ON t.note_id = n.id
                    WHERE t.tag = ?
                    ORDER BY n.id DESC
                    """,
                    (tag,),
                ).fetchall()
            return rows

    def search_notes(self, query: str) -> list[sqlite3.Row]:
        with self.connection() as conn:
            rows = conn.execute(
                """
                SELECT n.id, n.content, n.created_at
                FROM notes_fts f
                JOIN notes n ON n.id = f.rowid
                WHERE notes_fts MATCH ?
                ORDER BY n.id DESC
                """,
                (query,),
            ).fetchall()
            return rows

    def tag_note(self, note_id: int, tag: str) -> bool:
        with self.connection() as conn:
            exists = cast(
                sqlite3.Row | None,
                conn.execute("SELECT 1 FROM notes WHERE id = ?", (note_id,)).fetchone(),
            )
            if exists is None:
                return False
            _ = conn.execute(
                "INSERT OR IGNORE INTO note_tags(note_id, tag) VALUES(?, ?)",
                (note_id, tag),
            )
            return True

    def export_notes(self) -> list[dict[str, str | int]]:
        with self.connection() as conn:
            rows = conn.execute(
                "SELECT id, content, created_at FROM notes ORDER BY id DESC"
            ).fetchall()
            return [
                {
                    "id": int(row["id"]),
                    "content": str(row["content"]),
                    "created_at": str(row["created_at"]),
                }
                for row in rows
            ]

    def stats(self) -> dict[str, object]:
        with self.connection() as conn:
            total_notes = conn.execute("SELECT COUNT(*) AS count FROM notes").fetchone()
            tag_rows = conn.execute(
                """
                SELECT tag, COUNT(*) AS count
                FROM note_tags
                GROUP BY tag
                ORDER BY count DESC, tag ASC
                """
            ).fetchall()
            day_rows = conn.execute(
                """
                SELECT SUBSTR(created_at, 1, 10) AS day, COUNT(*) AS count
                FROM notes
                GROUP BY day
                ORDER BY day ASC
                """
            ).fetchall()

            return {
                "total_notes": int(total_notes["count"] if total_notes else 0),
                "tag_distribution": [
                    {"tag": str(row["tag"]), "count": int(row["count"])}
                    for row in tag_rows
                ],
                "notes_per_day": [
                    {"day": str(row["day"]), "count": int(row["count"])}
                    for row in day_rows
                ],
            }
