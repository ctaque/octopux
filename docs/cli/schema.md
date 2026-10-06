---
title: schema
parent: CLI
nav_order: 4
---

# schema

Displays the current SQL schema in your terminal.

In a terminal, a filter input above the tables keeps the ones whose table, column or constraint name contains what is typed; keywords separated by `,` or `|` add up (`book, page` or `book|page`). The arrows and the pages scroll, `Esc` quits. When piped, all the tables are drawn.

| Option | Description |
| --- | --- |
| `--migrations` | The migrations folder, `./migrations` by default |

```
$ octopux schema | cat
┌──────────────────────────┐    ┌───────────────────────────────┐
│ author                   │    │ book                          │
├──────────────────────────┤    ├───────────────────────────────┤
│ PK id   INTEGER          │    │ PK id        INTEGER          │
│    name TEXT    not null │    │    title     TEXT    not null │
└──────────────────────────┘    │    author_id INTEGER not null │
                                └───────────────────────────────┘


┌───────────────────────────────────┐
│ page                              │
├───────────────────────────────────┤
│ PK id          INTEGER            │
│    page_number INTEGER   not null │
│ FK book_id     INTEGER   not null │ ──▶ book.id
│    contents    TEXT      not null │
│    created_at  DATETIME?          │
│    updated_at  DATETIME?          │
│    deleted_at  DATETIME?          │
└───────────────────────────────────┘


Relations
  book  1──N  page  page.book_id

PK primary key · FK foreign key · uniq unique · ? nullable
```
