#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

from citation_audit import BibEntry, parse_bibtex, read_text, split_authors


def strip_tex(text: str) -> str:
    return (text or "").replace("{", "").replace("}", "").strip()


def parse_person(name: str) -> tuple[str, str]:
    name = strip_tex(name)
    if "," in name:
        last, given = [part.strip() for part in name.split(",", 1)]
        return given, last
    parts = name.split()
    if not parts:
        return "", ""
    if len(parts) == 1:
        return "", parts[0]
    return " ".join(parts[:-1]), parts[-1]


def initials(given: str) -> str:
    if not given:
        return ""
    return " ".join(f"{part[0]}." for part in given.replace("-", " ").split() if part)


def format_authors(entry: BibEntry, style: str) -> str:
    authors = split_authors(entry.get("author"))
    people = [parse_person(author) for author in authors]
    if not people:
        return "[Missing author]"

    if style == "apa":
        formatted = []
        for given, last in people:
            init = initials(given)
            formatted.append(f"{last}, {init}".strip().rstrip(","))
        if len(formatted) == 1:
            return formatted[0]
        if len(formatted) == 2:
            return f"{formatted[0]}, & {formatted[1]}"
        return ", ".join(formatted[:-1]) + f", & {formatted[-1]}"

    if style == "ieee":
        formatted = []
        for given, last in people:
            init = initials(given)
            formatted.append(f"{init} {last}".strip())
        return ", ".join(formatted)

    if style == "acm":
        formatted = []
        for given, last in people:
            formatted.append(f"{last}, {given}".strip().rstrip(","))
        if len(formatted) == 1:
            return formatted[0]
        if len(formatted) == 2:
            return f"{formatted[0]} and {formatted[1]}"
        return ", ".join(formatted[:-1]) + f", and {formatted[-1]}"

    if style == "gbt7714":
        formatted = []
        for given, last in people[:3]:
            init = initials(given).replace(" ", "")
            formatted.append(f"{last} {init}".strip())
        if len(people) > 3:
            formatted.append("et al")
        return ", ".join(formatted)

    raise ValueError(f"Unsupported style: {style}")


def format_container(entry: BibEntry) -> str:
    for field in ["journal", "booktitle", "publisher", "school", "institution"]:
        value = strip_tex(entry.get(field))
        if value:
            return value
    return "[Missing venue]"


def format_pages(entry: BibEntry) -> str:
    pages = strip_tex(entry.get("pages"))
    if not pages:
        return ""
    return pages.replace("--", "-")


def format_doi(entry: BibEntry, style: str) -> str:
    doi = strip_tex(entry.get("doi"))
    if not doi:
        return ""
    doi_url = f"https://doi.org/{doi}"
    if style == "ieee":
        return f"doi: {doi}"
    return doi_url


def format_article(entry: BibEntry, style: str) -> str:
    authors = format_authors(entry, style)
    title = strip_tex(entry.get("title")) or "[Missing title]"
    journal = strip_tex(entry.get("journal")) or "[Missing journal]"
    year = strip_tex(entry.get("year")) or "[Missing year]"
    volume = strip_tex(entry.get("volume"))
    number = strip_tex(entry.get("number")) or strip_tex(entry.get("issue"))
    pages = format_pages(entry)
    doi = format_doi(entry, style)

    if style == "apa":
        parts = [f"{authors} ({year}). {title}. {journal}"]
        if volume:
            vol = volume
            if number:
                vol += f"({number})"
            parts[-1] += f", {vol}"
        if pages:
            parts[-1] += f", {pages}"
        parts[-1] += "."
        if doi:
            parts.append(doi)
        return " ".join(parts)

    if style == "ieee":
        parts = [f"{authors}, \"{title},\" {journal}"]
        if volume:
            parts.append(f"vol. {volume}")
        if number:
            parts.append(f"no. {number}")
        if pages:
            parts.append(f"pp. {pages}")
        parts.append(year)
        if doi:
            parts.append(doi)
        return ", ".join(parts) + "."

    if style == "acm":
        parts = [f"{authors}. {year}. {title}. {journal}"]
        if volume:
            vol = volume
            if number:
                vol += f", {number}"
            parts[-1] += f" {vol}"
        if pages:
            parts[-1] += f", {pages}"
        parts[-1] += "."
        if doi:
            parts.append(doi)
        return " ".join(parts)

    if style == "gbt7714":
        parts = [f"{authors}. {title}[J]. {journal}, {year}"]
        if volume:
            vol = volume
            if number:
                vol += f"({number})"
            parts[-1] += f", {vol}"
        if pages:
            parts[-1] += f": {pages}"
        parts[-1] += "."
        if doi:
            parts.append(doi)
        return " ".join(parts)

    raise ValueError(f"Unsupported style: {style}")


def format_inproceedings(entry: BibEntry, style: str) -> str:
    authors = format_authors(entry, style)
    title = strip_tex(entry.get("title")) or "[Missing title]"
    booktitle = strip_tex(entry.get("booktitle")) or "[Missing proceedings]"
    year = strip_tex(entry.get("year")) or "[Missing year]"
    pages = format_pages(entry)
    doi = format_doi(entry, style)

    if style == "apa":
        text = f"{authors} ({year}). {title}. In {booktitle}"
        if pages:
            text += f" (pp. {pages})"
        text += "."
        return f"{text} {doi}".strip()

    if style == "ieee":
        parts = [f"{authors}, \"{title},\" in {booktitle}"]
        if pages:
            parts.append(f"pp. {pages}")
        parts.append(year)
        if doi:
            parts.append(doi)
        return ", ".join(parts) + "."

    if style == "acm":
        text = f"{authors}. {year}. {title}. In {booktitle}"
        if pages:
            text += f", {pages}"
        text += "."
        return f"{text} {doi}".strip()

    if style == "gbt7714":
        text = f"{authors}. {title}[C]//{booktitle}, {year}"
        if pages:
            text += f": {pages}"
        text += "."
        return f"{text} {doi}".strip()

    raise ValueError(f"Unsupported style: {style}")


def format_book(entry: BibEntry, style: str) -> str:
    authors = format_authors(entry, style)
    title = strip_tex(entry.get("title")) or "[Missing title]"
    publisher = strip_tex(entry.get("publisher")) or "[Missing publisher]"
    year = strip_tex(entry.get("year")) or "[Missing year]"
    doi = format_doi(entry, style)

    if style == "apa":
        text = f"{authors} ({year}). {title}. {publisher}."
    elif style == "ieee":
        text = f"{authors}, {title}. {publisher}, {year}."
    elif style == "acm":
        text = f"{authors}. {year}. {title}. {publisher}."
    elif style == "gbt7714":
        text = f"{authors}. {title}[M]. {publisher}, {year}."
    else:
        raise ValueError(f"Unsupported style: {style}")

    return f"{text} {doi}".strip()


def render_entry(entry: BibEntry, style: str) -> str:
    if entry.entry_type == "article":
        return format_article(entry, style)
    if entry.entry_type in {"inproceedings", "conference"}:
        return format_inproceedings(entry, style)
    if entry.entry_type == "book":
        return format_book(entry, style)

    authors = format_authors(entry, style)
    title = strip_tex(entry.get("title")) or "[Missing title]"
    year = strip_tex(entry.get("year")) or "[Missing year]"
    container = format_container(entry)
    doi = format_doi(entry, style)
    return f"{authors}. {year}. {title}. {container}. {doi}".strip()


def main() -> int:
    parser = argparse.ArgumentParser(description="Render a normalized BibTeX file into a base citation style.")
    parser.add_argument("--bib", required=True, help="Path to a .bib file")
    parser.add_argument("--style", required=True, choices=["apa", "ieee", "acm", "gbt7714"])
    parser.add_argument("--only", help="Optional comma-separated BibTeX keys to render")
    args = parser.parse_args()

    entries = parse_bibtex(read_text(Path(args.bib)))
    selected = {key.strip() for key in args.only.split(",")} if args.only else None

    for entry in entries:
        if selected and entry.key not in selected:
            continue
        print(f"[{entry.key}] {render_entry(entry, args.style)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
