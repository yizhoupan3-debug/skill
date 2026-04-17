#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


REQUIRED_FIELDS_BY_TYPE = {
    "article": ["author", "title", "journal", "year"],
    "inproceedings": ["author", "title", "booktitle", "year"],
    "conference": ["author", "title", "booktitle", "year"],
    "book": ["author", "title", "publisher", "year"],
    "inbook": ["author", "title", "publisher", "year"],
    "incollection": ["author", "title", "booktitle", "publisher", "year"],
    "phdthesis": ["author", "title", "school", "year"],
    "mastersthesis": ["author", "title", "school", "year"],
    "techreport": ["author", "title", "institution", "year"],
    "misc": ["author", "title", "year"],
    "unpublished": ["author", "title", "year"],
}

LATEX_CITE_PATTERN = re.compile(
    r"\\cite[a-zA-Z*]*\s*(?:\[[^\]]*\]\s*){0,2}\{([^}]*)\}", re.MULTILINE
)
PANDOC_CITE_PATTERN = re.compile(r"(?<![\w:-])@([A-Za-z0-9_:.+\-/]+)")


@dataclass
class BibEntry:
    entry_type: str
    key: str
    fields: dict[str, str] = field(default_factory=dict)

    def get(self, name: str) -> str:
        return self.fields.get(name.lower(), "").strip()

    @property
    def doi(self) -> str:
        return normalize_doi(self.get("doi"))

    @property
    def title_norm(self) -> str:
        return normalize_title(self.get("title"))

    @property
    def first_author_norm(self) -> str:
        authors = split_authors(self.get("author"))
        return normalize_person(authors[0]) if authors else ""

    @property
    def year(self) -> str:
        return self.get("year")


class BibTeXParseError(RuntimeError):
    pass


def normalize_whitespace(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def normalize_title(title: str) -> str:
    title = re.sub(r"[{}]", "", title or "")
    title = normalize_whitespace(title).casefold()
    return title


def normalize_doi(doi: str) -> str:
    doi = (doi or "").strip()
    doi = re.sub(r"^https?://(?:dx\.)?doi\.org/", "", doi, flags=re.I)
    doi = re.sub(r"^doi:\s*", "", doi, flags=re.I)
    return doi.strip().lower()


def normalize_person(name: str) -> str:
    name = re.sub(r"[{}]", "", name or "")
    return normalize_whitespace(name).casefold()


def split_authors(author_field: str) -> list[str]:
    if not author_field.strip():
        return []
    return [normalize_whitespace(part) for part in re.split(r"\s+and\s+", author_field) if part.strip()]


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def find_matching_delim(text: str, start: int, open_char: str, close_char: str) -> int:
    depth = 0
    i = start
    while i < len(text):
        ch = text[i]
        if ch == open_char:
            depth += 1
        elif ch == close_char:
            depth -= 1
            if depth == 0:
                return i
        elif ch == '"':
            i += 1
            while i < len(text):
                if text[i] == '"' and text[i - 1] != "\\":
                    break
                i += 1
        i += 1
    raise BibTeXParseError(f"Unmatched delimiter starting at offset {start}")


def parse_value(body: str, pos: int) -> tuple[str, int]:
    while pos < len(body) and body[pos].isspace():
        pos += 1
    if pos >= len(body):
        return "", pos

    if body[pos] == "{":
        end = find_matching_delim(body, pos, "{", "}")
        return body[pos + 1 : end], end + 1
    if body[pos] == '"':
        end = pos + 1
        escaped = False
        chunks = []
        while end < len(body):
            ch = body[end]
            if ch == '"' and not escaped:
                return "".join(chunks), end + 1
            if ch == "\\" and not escaped:
                escaped = True
            else:
                chunks.append(ch)
                escaped = False
            end += 1
        raise BibTeXParseError("Unterminated quoted BibTeX value")

    end = pos
    while end < len(body) and body[end] not in ",\n\r":
        end += 1
    return body[pos:end].strip(), end


def parse_fields(body: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    pos = 0
    length = len(body)
    while pos < length:
        while pos < length and body[pos] in " \t\r\n,":
            pos += 1
        if pos >= length:
            break

        name_start = pos
        while pos < length and re.match(r"[A-Za-z0-9_:-]", body[pos]):
            pos += 1
        field_name = body[name_start:pos].strip().lower()
        if not field_name:
            break

        while pos < length and body[pos].isspace():
            pos += 1
        if pos >= length or body[pos] != "=":
            raise BibTeXParseError(f"Expected '=' after field '{field_name}'")
        pos += 1
        value, pos = parse_value(body, pos)
        fields[field_name] = normalize_whitespace(value)

        while pos < length and body[pos] not in ",":
            if not body[pos].isspace():
                break
            pos += 1
        if pos < length and body[pos] == ",":
            pos += 1
    return fields


def parse_bibtex(text: str) -> list[BibEntry]:
    entries: list[BibEntry] = []
    pos = 0
    while True:
        at = text.find("@", pos)
        if at == -1:
            break
        type_match = re.match(r"@([A-Za-z]+)\s*([({])", text[at:])
        if not type_match:
            pos = at + 1
            continue
        entry_type = type_match.group(1).lower()
        opener = type_match.group(2)
        closer = ")" if opener == "(" else "}"
        body_start = at + type_match.end(2)
        body_end = find_matching_delim(text, body_start - 1, opener, closer)
        raw_body = text[body_start:body_end].strip()
        if "," not in raw_body:
            pos = body_end + 1
            continue
        key, fields_body = raw_body.split(",", 1)
        entries.append(BibEntry(entry_type=entry_type, key=key.strip(), fields=parse_fields(fields_body)))
        pos = body_end + 1
    return entries


def detect_missing_fields(entry: BibEntry) -> list[str]:
    required = REQUIRED_FIELDS_BY_TYPE.get(entry.entry_type, ["author", "title", "year"])
    return [field for field in required if not entry.get(field)]


def is_preprint(entry: BibEntry) -> bool:
    haystack = " ".join(
        [
            entry.entry_type,
            entry.get("journal"),
            entry.get("booktitle"),
            entry.get("archiveprefix"),
            entry.get("eprinttype"),
            entry.get("note"),
            entry.get("publisher"),
        ]
    ).lower()
    if any(token in haystack for token in ["arxiv", "biorxiv", "medrxiv", "ssrn", "preprint"]):
        return True
    return entry.entry_type in {"unpublished"}


def group_duplicates(entries: list[BibEntry]) -> list[list[BibEntry]]:
    buckets: dict[str, list[BibEntry]] = defaultdict(list)
    for entry in entries:
        if entry.doi:
            buckets[f"doi:{entry.doi}"].append(entry)
        else:
            signature = f"title:{entry.title_norm}|year:{entry.year}|author:{entry.first_author_norm}"
            buckets[signature].append(entry)
    return [group for group in buckets.values() if len(group) > 1]


def extract_manuscript_citation_keys(text: str) -> list[str]:
    keys: list[str] = []
    for match in LATEX_CITE_PATTERN.finditer(text):
        keys.extend([normalize_whitespace(part) for part in match.group(1).split(",") if part.strip()])
    for match in PANDOC_CITE_PATTERN.finditer(text):
        keys.append(match.group(1).strip())
    return keys


def sentence_split(text: str) -> list[str]:
    parts = re.split(r"(?<=[。！？!?\.])\s+|\n{2,}", text)
    return [normalize_whitespace(part) for part in parts if normalize_whitespace(part)]


def count_citations_in_sentence(sentence: str) -> int:
    count = 0
    for match in LATEX_CITE_PATTERN.finditer(sentence):
        count += len([part for part in match.group(1).split(",") if part.strip()])
    for match in re.finditer(r"\[(\d+(?:\s*[-,]\s*\d+)*)\]", sentence):
        payload = match.group(1)
        nums = [item for item in re.split(r"\s*,\s*", payload) if item.strip()]
        count += len(nums)
    for match in re.finditer(r"\(([^()]*(?:19|20)\d{2}[a-z]?[^()]*)\)$", sentence):
        payload = match.group(1)
        if ";" in payload:
            count += len([item for item in payload.split(";") if re.search(r"(?:19|20)\d{2}[a-z]?", item)])
    return count


def sentence_cluster_flags(text: str, threshold: int) -> list[dict[str, Any]]:
    flagged: list[dict[str, Any]] = []
    for sentence in sentence_split(text):
        count = count_citations_in_sentence(sentence)
        if count >= threshold:
            flagged.append(
                {
                    "citation_count": count,
                    "sentence": sentence,
                    "reason": f"sentence ends or contains a dense citation cluster ({count} citations detected)",
                }
            )
    return flagged


def make_report(entries: list[BibEntry], manuscript_text: str | None, cluster_threshold: int) -> dict[str, Any]:
    missing_fields = {
        entry.key: detect_missing_fields(entry) for entry in entries if detect_missing_fields(entry)
    }
    duplicate_groups = group_duplicates(entries)
    preprints = [entry.key for entry in entries if is_preprint(entry)]
    doi_missing = [entry.key for entry in entries if not entry.doi and entry.entry_type in {"article", "inproceedings", "conference"}]

    report: dict[str, Any] = {
        "summary": {
            "total_entries": len(entries),
            "duplicate_groups": len(duplicate_groups),
            "entries_with_missing_required_fields": len(missing_fields),
            "likely_preprints": len(preprints),
            "article_or_conference_entries_missing_doi": len(doi_missing),
        },
        "duplicates": [[entry.key for entry in group] for group in duplicate_groups],
        "missing_required_fields": missing_fields,
        "likely_preprints": preprints,
        "missing_doi": doi_missing,
    }

    if manuscript_text is not None:
        cited_keys = extract_manuscript_citation_keys(manuscript_text)
        cited_counter = Counter(cited_keys)
        bib_keys = {entry.key for entry in entries}
        missing_in_bib = sorted(set(cited_keys) - bib_keys)
        uncited_in_text = sorted(bib_keys - set(cited_keys))
        report["manuscript_consistency"] = {
            "total_in_text_citation_mentions": len(cited_keys),
            "unique_cited_keys": len(set(cited_keys)),
            "missing_in_bibliography": missing_in_bib,
            "uncited_reference_entries": uncited_in_text,
            "repeated_citation_keys": {k: v for k, v in cited_counter.items() if v > 1},
            "dense_citation_sentences": sentence_cluster_flags(manuscript_text, cluster_threshold),
        }

    return report


def report_to_markdown(report: dict[str, Any]) -> str:
    lines: list[str] = []
    summary = report["summary"]
    lines.append("# Citation Audit Report")
    lines.append("")
    lines.append("## Summary")
    for key, value in summary.items():
        lines.append(f"- **{key.replace('_', ' ')}**: {value}")

    lines.append("")
    lines.append("## Duplicate groups")
    if report["duplicates"]:
        for idx, group in enumerate(report["duplicates"], start=1):
            lines.append(f"- Group {idx}: {', '.join(group)}")
    else:
        lines.append("- None")

    lines.append("")
    lines.append("## Entries missing required fields")
    if report["missing_required_fields"]:
        for key, fields in report["missing_required_fields"].items():
            lines.append(f"- `{key}`: missing {', '.join(fields)}")
    else:
        lines.append("- None")

    lines.append("")
    lines.append("## Likely preprints")
    if report["likely_preprints"]:
        for key in report["likely_preprints"]:
            lines.append(f"- `{key}`")
    else:
        lines.append("- None")

    lines.append("")
    lines.append("## Entries missing DOI")
    if report["missing_doi"]:
        for key in report["missing_doi"]:
            lines.append(f"- `{key}`")
    else:
        lines.append("- None")

    consistency = report.get("manuscript_consistency")
    if consistency:
        lines.append("")
        lines.append("## Manuscript consistency")
        lines.append(f"- **total in-text citation mentions**: {consistency['total_in_text_citation_mentions']}")
        lines.append(f"- **unique cited keys**: {consistency['unique_cited_keys']}")

        lines.append("")
        lines.append("### Missing in bibliography")
        if consistency["missing_in_bibliography"]:
            for key in consistency["missing_in_bibliography"]:
                lines.append(f"- `{key}`")
        else:
            lines.append("- None")

        lines.append("")
        lines.append("### Uncited reference entries")
        if consistency["uncited_reference_entries"]:
            for key in consistency["uncited_reference_entries"]:
                lines.append(f"- `{key}`")
        else:
            lines.append("- None")

        lines.append("")
        lines.append("### Dense citation sentences")
        dense = consistency["dense_citation_sentences"]
        if dense:
            for item in dense:
                lines.append(f"- ({item['citation_count']} cites) {item['sentence']}")
        else:
            lines.append("- None")

    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit BibTeX references and optional manuscript citation consistency.")
    parser.add_argument("--bib", required=True, help="Path to a .bib file")
    parser.add_argument("--manuscript", help="Optional manuscript text (.tex/.md/.txt) to scan for citations")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    parser.add_argument("--cluster-threshold", type=int, default=3, help="Flag sentences with at least this many citations")
    args = parser.parse_args()

    bib_path = Path(args.bib)
    entries = parse_bibtex(read_text(bib_path))
    manuscript_text = read_text(Path(args.manuscript)) if args.manuscript else None
    report = make_report(entries, manuscript_text, args.cluster_threshold)

    if args.format == "json":
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        print(report_to_markdown(report), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
