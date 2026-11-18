# Notes specification and test vectors

The Mac app is the specification (`../menew/Sources/Menew`). This file lists, for the notes, what the Mac
code does with a given input and where in the Mac code that comes from. The tables in "Vectors" are
**executed**: `crates/menew-core/tests/notes_spec.rs` reads this file, runs every row through the real Linux
code and compares the result. A row that does not match fails the test, and the test also fails if it executes
a different number of rows than the file has. The expected values were derived by hand from the Mac code, not
copied from the Linux output.

In a table cell `\n` stands for a line break, `-` is "nothing / none", `(e)` is an empty string, and
`REPEAT:a:80` is the letter `a` 80 times. Cells never contain a pipe character.

## Where each part comes from in the Mac code

| Part | Mac file / function | Linux |
|---|---|---|
| Note model, title, preview, `#tags` | `Notes.swift` `Note`, `Note.tags(in:)` | `notes.rs` `Note`, `markdown.rs` `tags_in` |
| Storage format (`notes.json`, dates as seconds since 2001) | `Notes.swift` `NotesStore.save`, `Codable` | `notes.rs` `Note` (serde) |
| Password rows (`key: value`) parse and format | `Notes.swift` `SecretField.parse`, `format` | `notes.rs` `SecretField` |
| Pick password / username in any language | `ClipboardManager.swift` `secretValue`; test `testPicksPasswordAndUsernameInAnyLanguage` | `notes.rs` `secret_value` |
| Test ports | `MenewTests.swift` `testParseKeepsTitleAndFields`, `testFormatRoundTrip`, `testPicksPasswordAndUsernameInAnyLanguage` | rows B01, C01, D01 to D06 (literal) |
| Markdown blocks | `Markdown.swift` `MarkdownView.blocks` | `markdown.rs` `blocks` |
| Markdown inline | `Markdown.swift` `inline` (Foundation inline Markdown) | `markdown.rs` `inline_pango` |
| Tick a checklist | `NotesViews.swift` `NoteEditor.toggleTask` | `markdown.rs` `toggle_task` |
| Search, tag filter, order | `Notes.swift` `NotesStore.filtered`, `allTags` | `notes.rs` `NotesStore` |
| Protect, unprotect, type switch | `Notes.swift` `setSecure`, `setKind`, `NoteVault` | `notes.rs` `NotesStore`, `Vault` |
| Notes in the palette | `Palette.swift` `commands` (`Module.notes`) | `palette.rs` `commands` |
| Passwords filter of the clipboard | `ClipboardManager.swift` `passwordItems`, `pasteSecret` | `notes.rs` `password_notes` |
| Copied secret: confidential, cleared after 30 s | `Notes.swift` `SecretCopy.copy` | `menew-ui` `copy_secret`, `notes.rs` `secret_clear_secs` |
| Strings | `L10n.swift` | `strings.rs` |

## Differences (also in `docs/parity.md`)

- **Touch ID** becomes the password of the user asked through polkit (`app.menew.Menew.unlock-note`, `auth_self`);
  without the policy or an agent the keyring unlocked at login is the only protection and the UI says so.
- **Keychain** becomes the Secret Service (`application=app.menew.Menew`, `note=<uuid>`).
- **`NSPasteboard ConcealedType`** becomes the type `x-kde-passwordManagerHint` with the value `secret`
  (the daemon skips any type with "password" in its name, and so do other managers).
- The Mac clears the clipboard if `changeCount` is unchanged after 30 s; GTK has no change counter, so Linux
  clears it if **the content provider Menew set is still the clipboard content** (rows K01 to K05 test the delay).
- Inline Markdown is written by hand (bold, italic, code, strikethrough, links, tags) instead of Foundation's
  parser; only `http`, `https` and `mailto` links are clickable (Foundation lets any scheme through).
- `\p{L}` in tags is Rust's `is_alphabetic`, a few marks differ. A title is cut at 80 `char`s, not graphemes.
- Linux-only: palette actions "New note" and "New passwords note" (the Mac has them as buttons in the notes tab).
- Not in release 1: summarize / extract to-dos with AI (see `docs/parity.md`).

## Vectors

### A. Title, preview and tags

Columns: id, note text, title, preview, tags (lowercase, in order). Language English.

| id | text | title | preview | tags |
|---|---|---|---|---|
| A01 | Hello\nworld | Hello | world | - |
| A02 | # Menew ideas\n- [x] Snippets\n- [ ] Per-app | Menew ideas | Snippets | - |
| A03 | \n\n  ## Title  \n> quote *x* | Title | quote x | - |
| A04 | (e) | Untitled note | (e) | - |
| A05 | ### | Untitled note | (e) | - |
| A06 | Meeting\n#work and #Home #work-2\nc#no | Meeting | work and #Home #work-2 | work,home,work-2 |
| A07 | # Title with space\n# heading not tag | Title with space | heading not tag | - |
| A08 | x\n1) one | x | one | - |
| A09 | x\n12. twelve | x | twelve | - |
| A10 | x\n+ plus | x | plus | - |
| A11 | x\n`code` and ~~s~~ _u_ | x | code and s u | - |
| A12 | REPEAT:a:100 | REPEAT:a:80 | (e) | - |
| A13 | x\n#café #日本 #a_b | x | café #日本 #ab | café,日本,a_b |
| A14 | x\nfoo #bar#baz | x | foo #bar#baz | bar |
| A15 | #A #a | A #a | (e) | a |

### B. Password rows: parse

Columns: id, text, title, names, values. Lists are joined with `¶`.

| id | text | title | names | values |
|---|---|---|---|---|
| B01 | Banco\nUsuário: ana\nSenha: a:b:c\n\nPIN: | Banco | Usuário¶Senha¶PIN | ana¶a:b:c¶(e) |
| B02 | Title only | Title only | - | - |
| B03 | T\nno colon line | T | no colon line | (e) |
| B04 | T\n  key  :  two spaces | T | key | ` two spaces` |
| B05 | T\nk:v | T | k | v |
| B06 | (e) | (e) | - | - |
| B07 | T\n   \n  \nk: v | T | k | v |

### C. Password rows: format

Columns: id, title, fields (`name=value` joined by `¶`), the text written.

| id | title | fields | text |
|---|---|---|---|
| C01 | Wi-Fi | Usuário=ana¶Senha=x y | Wi-Fi\nUsuário: ana\nSenha: x y |
| C02 | A\nB | k:ey=va\nl | A B\nkey: va l |
| C03 | T | PIN= | T\nPIN: |
| C04 | T | - | T |

The round trip of C01 (`parse(format(...))`) gives back the same title, names and values (Mac test `testFormatRoundTrip`).

### D. Password and username by name, in any language

Columns: id, fields, password, username. The first six rows are the cases of the Mac test
`testPicksPasswordAndUsernameInAnyLanguage`, in the same order.

| id | fields | password | username |
|---|---|---|---|
| D01 | Usuário=ana@x.com¶Senha=s3nha | s3nha | ana@x.com |
| D02 | Username=ana¶Password=pw1 | pw1 | ana |
| D03 | Benutzer=ana¶Passwort=pw2 | pw2 | ana |
| D04 | Usuario=ana¶Contraseña=pw3 | pw3 | ana |
| D05 | a=first¶b=last | last | first |
| D06 | PIN=1234 | 1234 | - |
| D07 | 账号=ana¶密码=pw5 | pw5 | ana |
| D08 | Login=bob¶Senha= | (e) | bob |
| D09 | Rede=Casa¶Senha=abc¶PIN=1 | abc | Casa |
| D10 | Site=x.com¶Key=k1¶Pass=p2 | k1 | x.com |
| D11 | E-mail=a@b.c¶SENHA=zz | zz | a@b.c |
| D12 | Account=acc¶Contabilidade=7 | 7 | acc |

### E. Markdown blocks

Columns: id, text, blocks joined by ` ¦ `. Block syntax: `H<level>:text`, `T+<line>:text` (ticked) and `T-<line>:text`,
`B<indent>:text`, `N<number>:text`, `Q:text`, `C:code`, `R` (rule), `P:text`, `S` (space).

| id | text | blocks |
|---|---|---|
| E01 | # Title\n\nText **b**\n- [ ] todo\n- [x] done\n- item\n  - nested\n1. one\n2) two\n> quote\n---\n```\ncode\n  indented\n```\nend | H1:Title ¦ S ¦ P:Text **b** ¦ T-3:todo ¦ T+4:done ¦ B0:item ¦ B1:nested ¦ N1:one ¦ N2:two ¦ Q:quote ¦ R ¦ C:code\n  indented ¦ P:end |
| E02 | \n\nA\n\n\nB\n\n | P:A ¦ S ¦ P:B ¦ S |
| E03 | #### four\n#nospace\n# \n#   x | P:#### four ¦ P:#nospace ¦ P:# ¦ H1:x |
| E04 | - [X] Big\n* [ ]\n+ plain\n-no\n- [x]tight | T+0:Big ¦ T-1: ¦ B0:plain ¦ P:-no ¦ T+4:tight |
| E05 | ```\nabc\ndef | C:abc\ndef |
| E06 | ```rust\nx\n```\ny | C:x ¦ P:y |
| E07 | > a\n>b\n>\n>   c | Q:a ¦ Q:b ¦ Q: ¦ Q:  c |
| E08 | ***\n---\n--\n- - - | R ¦ R ¦ P:-- ¦ B0:- - |
| E09 | 10. ten\n1.x\n3)  three | N10:ten ¦ P:1.x ¦ N3:three |
| E10 | (e) | - |
| E11 |    spaced    \n    - b\n   - c | P:spaced ¦ B2:b ¦ B1:c |

The `E02` row shows a quirk kept from the Mac: a blank line is dropped when the line before it (in the list before
filtering) is blank, or when it is the first or last line, so one trailing blank line can stay.

### F. Tick a checklist (`toggleTask`)

Columns: id, text, line (0-based), result.

| id | text | line | result |
|---|---|---|---|
| F01 | - [ ] a\n- [x] b | 0 | - [x] a\n- [x] b |
| F02 | - [ ] a\n- [x] b | 1 | - [ ] a\n- [ ] b |
| F03 | - [X] c | 0 | - [ ] c |
| F04 | - [ ] a [x] b | 0 | - [x] a [x] b |
| F05 | - [x] a [ ] b | 0 | - [x] a [x] b |
| F06 | plain | 0 | plain |
| F07 | - [ ] a | 9 | - [ ] a |

### G. Inline Markdown as Pango markup

Columns: id, input, output.

| id | input | output |
|---|---|---|
| G01 | plain <b> & text | plain &lt;b&gt; &amp; text |
| G02 | **bold** and *it* and `code` | <b>bold</b> and <i>it</i> and <tt>code</tt> |
| G03 | ~~gone~~ | <s>gone</s> |
| G04 | [site](https://example.com) | <a href="https://example.com">site</a> |
| G05 | see #Work and #a-b | see <a href="menew-tag://work">#Work</a> and <a href="menew-tag://a-b">#a-b</a> |
| G06 | x#no | x#no |
| G07 | snake_case_name | snake_case_name |
| G08 | [x](javascript:alert) | [x](javascript:alert) |
| G09 | **#tag** | <b>#tag</b> |
| G10 | a \*lit\* b | a *lit* b |
| G11 | unclosed **bold | unclosed **bold |
| G12 | 2 * 3 * 4 | 2 * 3 * 4 |

`G08` is a deliberate difference: Foundation would make a link of any scheme.

### H. Notes in the palette

Columns: id, notes (`id:kind:text`, joined by `¶`; kinds `text`, `secure`, `passwords`; a protected note has only its
title in `text`), query, ids of the note items found (in order), subtitle of the first note item.

| id | notes | query | found | subtitle |
|---|---|---|---|---|
| H01 | a:text:Groceries\ncoffee and bread¶b:text:Meeting\nbudget | coffee | note.a | coffee and bread |
| H02 | a:text:Groceries\ncoffee and bread¶b:text:Meeting\nbudget | budget | note.b | budget |
| H03 | c:secure:Bank | bank | note.c | Protected content |
| H04 | a:text:Groceries\ncoffee | zzzz | - | - |
| H05 | d:passwords:Home Wi-Fi | home | note.d | Protected content |
| H06 | e:text:Reunião\nplanejamento | reuniao | note.e | planejamento |

### I. Clipboard "Passwords" filter

Columns: id, notes (`id:kind:text:updated`, joined by `¶`), query, ids in order.

| id | notes | query | ids |
|---|---|---|---|
| I01 | a:passwords:Wi-Fi:1¶b:passwords:Bank:3¶c:text:Bank notes:5¶d:secure:Vault:4 | (e) | b,a |
| I02 | a:passwords:Wi-Fi:1¶b:passwords:Bank:3¶c:text:Bank notes:5 | bank | b |
| I03 | a:passwords:Wi-Fi:1¶b:passwords:Bank:3 | WI | a |
| I04 | a:passwords:Wi-Fi:1 | x | - |

### K. Seconds until a copied secret is cleared

Columns: id, test mode (`yes`/`no`), `MENEW_SECRET_CLEAR_SECS`, seconds. The Mac clears after 30 s
(`SecretCopy.copy`); the variable only counts with `MENEW_TEST=1`.

| id | test mode | override | seconds |
|---|---|---|---|
| K01 | no | 3 | 30 |
| K02 | yes | 3 | 3 |
| K03 | yes | - | 30 |
| K04 | yes | 0 | 30 |
| K05 | yes | abc | 30 |

### L. The file of the Mac

Columns: id, the JSON of `notes.json` (dates are `Date` seconds since 2001-01-01), then
`id,secure,kind,created_unix,updated_unix` of the first note. A note without `kind` is a text note, and writing it
back leaves `kind` out like Swift's `encodeIfPresent`.

| id | json | first note |
|---|---|---|
| L01 | [{"id":"AAAAAAAA-0000-4000-8000-000000000001","text":"Hello #tag","secure":false,"created":0,"updated":86400}] | AAAAAAAA-0000-4000-8000-000000000001,false,text,978307200,978393600 |
| L02 | [{"id":"BBBBBBBB-0000-4000-8000-000000000002","text":"Wi-Fi","secure":true,"created":1.5,"updated":2.5,"kind":"passwords"}] | BBBBBBBB-0000-4000-8000-000000000002,true,passwords,978307201.5,978307202.5 |
| L03 | [{"id":"CCCCCCCC-0000-4000-8000-000000000003","text":"x","secure":false,"created":10,"updated":10,"kind":"text"}] | CCCCCCCC-0000-4000-8000-000000000003,false,text,978307210,978307210 |

## Not covered by vectors (checked by the desktop tests, `scripts/desktop-test/phase6.sh`)

Secret Service round trip (`secret-tool lookup application app.menew.Menew note <uuid>`), no secret in the plain
files (`grep -r`), the confidential type on the clipboard, the clipboard cleared after the delay only when it still
holds the secret, polkit with and without an agent, the notes in the palette and the Passwords filter in the real daemon.
