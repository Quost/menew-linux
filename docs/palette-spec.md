# Palette specification and test vectors

The Mac app is the specification (`../menew/Sources/Menew`). This file lists, for the command
palette, what the Mac code does with a given input and where in the Mac code that comes from. The
table in "Vectors" is **executed**: `crates/menew-core/tests/palette_spec.rs` reads this file, runs
every row through the real palette engine (`menew_core::palette::results`) and compares the result.
A row that does not match fails the test, and the test also fails if it executes a different number
of rows than the file has. The expected values were derived by hand from the Mac code, not copied
from the Linux output.

## Fixed environment of the vectors

| What | Value |
|---|---|
| Now | 2026-11-14 12:00:00 UTC (Saturday), Unix `1794657600` |
| This computer's zone | `America/Sao_Paulo` (UTC-3, no DST): 09:00 here |
| Process zone (`TZ`) | `UTC` (used for dates inside a JWT) |
| Apps | Firefox (`firefox`), Microsoft Teams (`teams`) |
| Quick links | the six defaults plus `jira` = `https://company.atlassian.net/browse/{q}` |
| Timers running | none |

`en` is used unless the row says another language. In a Markdown table cell `\n` stands for a line
break and ` ` for a no-break space. `*` means "not checked", `(none)` is an empty
subtitle, `(empty)` is an empty input, `-` is "no clipboard text" or "no recents".
In the "Item" column, `id` is an item id that must be in the results, `!id` is an id that must **not**
be there and `@N` is the item at position N (0-based).

## Where each part comes from in the Mac code

| Part | Mac file / function | Linux |
|---|---|---|
| Search and scores (exact 1000, prefix, word, initials, contains, keywords, subsequence) | `Palette.swift` `PaletteSearch.score` | `search.rs` `score` |
| Order of results, recents bonus, 40 results, web search last | `Palette.swift` `PaletteModel.update` | `palette.rs` `results` |
| Math, percent, functions, `2(3+1)`, `200+10%` | `Calculator.swift` `evaluate`, `Parser` | `calculator.rs` |
| Units and conversions | `Calculator.swift` `units`, `convert`, `splitConversion` | `conversions.rs` |
| Currency (ECB rate through frankfurter.app, only on Enter) | `Calculator.swift` `currencyQuery`, `fetchRate`; `Palette.swift` `calculatorItems` | `currency.rs`, `palette.rs` |
| Time zones | `WorldClock.swift` `paletteQuery`, `zone`, `offset`, `dayNote` | `time_zones.rs` |
| Unix timestamp | `Palette.swift` `devItems` | `dev_tools.rs` |
| Text transforms | `TextTransforms.swift` | `text_transforms.rs` |
| Quick links | `QuickLinks.swift` | `quick_links.rs` |
| Emoji and symbols | `Emoji.swift` | `emoji.rs`, `emoji_data.rs` |
| Timers and alarms | `Timers.swift` | `timers.rs` |
| Focus | `Focus.swift` | `focus.rs` |
| Script commands | `ScriptCommands.swift` | `script_commands.rs` |
| Recent picks | `Palette.swift` `remember`, `recents` | `recents.rs` |
| Strings | `L10n.swift` | `strings.rs` |

## Known differences (also in `docs/parity.md`)

- Dates and currency use locale rules written by hand for the 5 languages (`format.rs`), not
  Foundation. The exact text of a long date can differ from what macOS prints.
- Pretty-printed JSON follows Apple's style (`"key" : value`, 2 spaces, sorted keys); an empty
  collection prints as `[]` / `{}`. The Mac output for empty collections was not checked.
- Word counts use runs of letters and digits, not Apple's word breaker (differs for Chinese and Japanese).
- The emoji list is generated from the Unicode data of the build host with the Mac's filter; the
  Mac uses the data of its macOS version, so a few recent emoji can differ.
- `emoji` words in other languages: the Mac resolves the prefix with `Dictionary.first`, whose
  order is random per run; Linux takes the first entry of the table.
- Symbol titles come from the first Portuguese keyword on the Mac ("Seta Direita"); Linux keeps that.
- The Apple logo symbol is left out.

## Vectors

| ID | Lang | Clipboard | Recents | Input | Item | Title | Subtitle | Action | Mac source |
|---|---|---|---|---|---|---|---|---|---|
| V001 | en | - | - | 12*7.5 | calc | 90 | 12*7.5 | copy:90 | Calculator.evaluate |
| V002 | pt | - | - | 12*7,5 | calc | 90 | 12*7,5 | copy:90 | Calculator.number (comma is a decimal point) |
| V003 | en | - | - | 20% of 340 | calc | 68 | 20% of 340 | copy:68 | Calculator.percentOf |
| V004 | pt | - | - | 20% de 340 | calc | 68 | 20% de 340 | copy:68 | Calculator.percentOf ("de") |
| V005 | de | - | - | 20% von 340 | calc | 68 | 20% von 340 | copy:68 | Calculator.percentOf ("von") |
| V006 | zh | - | - | 20% 的 340 | calc | 68 | 20% 的 340 | copy:68 | Calculator.percentOf ("的") |
| V007 | en | - | - | 5 km to mi | calc | 3.1069 mi | 5 km | copy:3.1069 mi | Calculator.convert |
| V008 | pt | - | - | 5 km em mi | calc | 3,1069 mi | 5 km | copy:3,1069 mi | Calculator.convert, locale decimal |
| V009 | en | - | - | 72 f to c | calc | 22.2222 °C | 72 °F | copy:22.2222 °C | Calculator.convert (temperature) |
| V010 | en | - | - | 72f to c | calc | 22.2222 °C | 72 °F | copy:22.2222 °C | Calculator.splitConversion (number glued to the unit) |
| V011 | pt | - | - | 1,5 gb em mb | calc | 1500 MB | 1,5 GB | copy:1500 MB | Calculator.convert, Calculator.symbol (data) |
| V012 | en | - | - | 10 cm in in | calc | 3.937 in | 10 cm | copy:3.937 in | Calculator.splitConversion (connector is the 3rd word) |
| V013 | en | - | - | 2^8 | calc | 256 | 2^8 | copy:256 | Parser.power |
| V014 | en | - | - | sqrt 16 | calc | 4 | sqrt 16 | copy:4 | Parser.primary (function) |
| V015 | en | - | - | 200 + 10% | calc | 220 | 200 + 10% | copy:220 | Parser.expr (percent after + is of the left value) |
| V016 | en | - | - | 5! | calc | 120 | 5! | copy:120 | Parser.postfix (factorial) |
| V017 | en | - | - | 2pi | calc | 6.2831853072 | 2pi | copy:6.2831853072 | Parser.term (implicit multiplication) |
| V018 | en | - | - | 2(3+1) | calc | 8 | 2(3+1) | copy:8 | Parser.term |
| V019 | en | - | - | 1/0 | !calc | * | * | * | Calculator.evaluate (isFinite) |
| V020 | en | - | - | 100 usd to brl | calc.fx.fetch | Convert $100.00 to BRL | ↩ fetches today's rate (frankfurter.app) | fetch-rate:100 USD->BRL | Palette.calculatorItems (no request while typing) |
| V021 | pt | - | - | 100 usd em brl | calc.fx.fetch | Converter US$ 100,00 em BRL | ↩ busca a cotação do dia (frankfurter.app) | fetch-rate:100 USD->BRL | Palette.calculatorItems, Calculator.currencyText |
| V022 | en | - | - | 3pm ny to lisbon | clock.Europe/Lisbon | 8:00 PM in Lisbon | 3:00 PM in New York · +5 h | copy:8:00 PM | WorldClock.paletteQuery, Palette.devItems |
| V023 | pt | - | - | 15h em Lisboa | clock.Europe/Lisbon | 18:00 em Lisboa | 15:00 aqui · +3 h | copy:18:00 | WorldClock.paletteQuery |
| V024 | en | - | - | time in Tokyo | clock.Asia/Tokyo | 9:00 PM in Tokyo | 9:00 AM here · +12 h | copy:9:00 PM | WorldClock.paletteQuery (hour words) |
| V025 | pt | - | - | hora em Tóquio | clock.Asia/Tokyo | 21:00 em Tóquio | 09:00 aqui · +12 h | copy:21:00 | WorldClock.paletteQuery, WorldClock.displayNames |
| V026 | pt | - | - | lisboa | clock.Europe/Lisbon | 12:00 em Lisboa | 09:00 aqui · +3 h | copy:12:00 | WorldClock.paletteQuery (only the city) |
| V027 | en | - | - | 11pm ny to tokyo | clock.Asia/Tokyo | 1:00 PM in Tokyo | 11:00 PM in New York · +14 h · tomorrow | copy:1:00 PM | WorldClock.dayNote |
| V028 | en | - | - | 10pm tokyo to ny | clock.America/New_York | 8:00 AM in New York | 10:00 PM in Tokyo · −14 h | copy:8:00 AM | WorldClock.offset (U+2212 minus) |
| V029 | de | - | - | 15h in Lissabon | clock.Europe/Lisbon | 18:00 in Lissabon | 15:00 hier · +3 h | copy:18:00 | WorldClock.aliases ("lissabon") |
| V030 | es | - | - | 15h en Lisboa | clock.Europe/Lisbon | 18:00 en Lisboa | 15:00 aquí · +3 h | copy:18:00 | WorldClock.paletteQuery ("en") |
| V031 | zh | - | - | 15h 到 里斯本 | clock.Europe/Lisbon | 里斯本 18:00 | 此处 15:00 · +3 h | copy:18:00 | WorldClock.paletteQuery ("到"); L10n "%@ em %@" (zh: %2$@ %1$@) |
| V032 | en | - | - | 1793400000 | timestamp.date | Oct 30, 2026 at 7:40:00 PM | Unix timestamp · ↩ copies 2026-10-30T22:40:00Z | copy:2026-10-30T22:40:00Z | Palette.devItems (10 digits are seconds) |
| V033 | en | - | - | 1793400000123 | timestamp.date | Oct 30, 2026 at 7:40:00 PM | Unix timestamp · ↩ copies 2026-10-30T22:40:00Z | copy:2026-10-30T22:40:00Z | Palette.devItems (13 digits are milliseconds) |
| V034 | en | - | - | 179340000 | !timestamp.date | * | * | * | Palette.devItems (9 digits is not a timestamp) |
| V035 | en | hello World | - | snake_case | text.snake | snake_case | hello_world | transform:snake | TextTransform.snake |
| V036 | en | helloWorld | - | kebab | text.kebab | kebab-case | hello-world | transform:kebab | TextTransform.kebab, identifiers |
| V037 | en | hello world | - | camel | text.camel | camelCase | helloWorld | transform:camel | TextTransform.camel |
| V038 | en | HTTPServer error | - | snake | text.snake | snake_case | http_server_error | transform:snake | TextTransform.identifiers ("HTTPServer" is HTTP Server) |
| V039 | en | hello world | - | upper | text.upper | UPPERCASE | HELLO WORLD | transform:upper | TextTransform.upper |
| V040 | en | Hello WORLD | - | lower | text.lower | lowercase | hello world | transform:lower | TextTransform.lower |
| V041 | en | hello wORLD | - | title case | text.title | Title Case | Hello World | transform:title | TextTransform.title |
| V042 | en | hello. WORLD is here | - | sentence | text.sentence | Sentence case | Hello. World is here | transform:sentence | TextTransform.sentenceCase |
| V043 | en | Ação é útil | - | accents | text.noAccents | Remove accents | Acao e util | transform:noAccents | TextTransform.noAccents |
| V044 | en | a   b \n\n\n c  | - | extra spaces | text.trim | Remove extra spaces | a b ⏎ c | transform:trim | TextTransform.collapseWhitespace |
| V045 | en | b\na\nc | - | sort | text.sortLines | Sort lines | a ⏎ b ⏎ c | transform:sortLines | TextTransform.sortLines |
| V046 | en | file10\nfile2\nFile1 | - | sort | text.sortLines | Sort lines | File1 ⏎ file2 ⏎ file10 | transform:sortLines | localizedStandardCompare (numbers by value) |
| V047 | en | a\nb\nc | - | reverse | text.reverseLines | Reverse lines | c ⏎ b ⏎ a | transform:reverseLines | TextTransform.reverseLines |
| V048 | en | a\nb\na | - | dedupe | text.uniqueLines | Remove duplicate lines | a ⏎ b | transform:uniqueLines | TextTransform.uniqueLines |
| V049 | en | {"b":1,"a":[1,2]} | - | format json | text.jsonPretty | Format JSON | { ⏎   "a" : [ ⏎     1, ⏎     2 ⏎   ], ⏎   "b" : 1 ⏎ } | transform:jsonPretty | TextTransform.json (prettyPrinted, sortedKeys) |
| V050 | en | { "b": 1, "a": [1, 2] } | - | minify json | text.jsonMinify | Minify JSON | {"a":[1,2],"b":1} | transform:jsonMinify | TextTransform.json (compact) |
| V051 | en | {"a":} | - | format json | text.jsonPretty | Format JSON | Invalid JSON | transform:jsonPretty | TextTransform.failure |
| V052 | en | hello | - | base64 | text.base64Encode | Encode Base64 | aGVsbG8= | transform:base64Encode | TextTransform.base64Encode |
| V053 | en | aGVsbG8= | - | base64 | text.base64Decode | Decode Base64 | hello | transform:base64Decode | TextTransform.base64Decode |
| V054 | en | aGVsbG8 | - | base64 | text.base64Decode | Decode Base64 | hello | transform:base64Decode | base64Decode (padding is optional) |
| V055 | en | not base64! | - | base64 | text.base64Decode | Decode Base64 | Not valid Base64 | transform:base64Decode | TextTransform.failure |
| V056 | en | a b&c | - | url | text.urlEncode | URL-encode | a%20b%26c | transform:urlEncode | TextTransform.urlEncode (unreserved set) |
| V057 | en | a%20b%26c+d | - | url | text.urlDecode | URL-decode | a b&c d | transform:urlDecode | TextTransform.urlDecode ("+" is a space) |
| V058 | en | Hello world\nline two | - | count | text.count | Count characters and words | 20 characters · 4 words · 2 lines | transform:count | TextTransform.stats |
| V059 | en | one | - | count | text.count | Count characters and words | 3 characters · 1 words · 1 line | transform:count | TextTransform.stats (singular line) |
| V060 | en | eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIiwiZXhwIjoxNzkzNDAwMDAwfQ.sig | - | jwt | text.jwt | Decode JWT | { ⏎   "alg" : "HS256", ⏎   "typ" : "JWT" ⏎ } ⏎ { ⏎   "exp" : 1793400000, ⏎   "sub" : "1" ⏎ } ⏎ exp: 10/30/2026, 10:40 PM | transform:jwt | TextTransform.jwt (dates of exp, iat, nbf) |
| V061 | en | Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIiwiZXhwIjoxNzkzNDAwMDAwfQ.sig | - | jwt | text.jwt | Decode JWT | * | transform:jwt | TextTransform.jwt ("Bearer " is dropped) |
| V062 | en | not.a.jwt | - | jwt | text.jwt | Decode JWT | Not a valid JWT | transform:jwt | TextTransform.failure |
| V063 | pt | olá mundo | - | maiusculas | text.upper | MAIÚSCULAS | OLÁ MUNDO | transform:upper | L10n "MAIÚSCULAS", PaletteSearch.fold |
| V064 | en | - | - | uuid | action.uuid | Generate UUID | * | generate-uuid | Palette.commands "uuid" |
| V065 | en | - | - | timestamp | action.timestamp | Copy Unix timestamp | * | copy-unix-timestamp | Palette.commands "timestamp" |
| V066 | en | - | - | g coffee | link.g | Google: coffee | https://www.google.com/search?q=coffee | open:https://www.google.com/search?q=coffee | QuickLinkStore.paletteItems |
| V067 | en | - | - | g coffee | !link.search | * | * | * | QuickLinkStore.webSearchItem (a keyword hides web search) |
| V068 | en | - | - | gh menew | link.gh | GitHub: menew | https://github.com/search?q=menew | open:https://github.com/search?q=menew | QuickLinkStore.defaults |
| V069 | en | - | - | maps paulista | link.maps | OpenStreetMap: paulista | https://www.openstreetmap.org/search?query=paulista | open:https://www.openstreetmap.org/search?query=paulista | QuickLinkStore.defaults (Linux: OpenStreetMap instead of Apple Maps) |
| V070 | en | - | - | maps avenida paulista | link.maps | OpenStreetMap: avenida paulista | https://www.openstreetmap.org/search?query=avenida%20paulista | open:https://www.openstreetmap.org/search?query=avenida%20paulista | QuickLink.url(for:) |
| V071 | en | - | - | jira ABC-123 | link.jira | Jira: ABC-123 | https://company.atlassian.net/browse/ABC-123 | open:https://company.atlassian.net/browse/ABC-123 | custom link keeps the capitals of the typed text |
| V072 | en | - | - | g | link.g | Google: type what to search | https://www.google.com/search?q={q} | none | QuickLinkStore.paletteItems (needsText and no text) |
| V073 | en | - | - | github.com/quost | link.open | Open github.com | https://github.com/quost | open:https://github.com/quost | QuickLinkStore.webAddress |
| V074 | en | - | - | coffee shop | link.search | Search “coffee shop” on Google | https://www.google.com/search?q=coffee%20shop | open:https://www.google.com/search?q=coffee%20shop | QuickLinkStore.webSearchItem |
| V075 | en | - | - | wiki ação | link.wiki | Wikipedia: ação | https://wikipedia.org/w/index.php?search=a%C3%A7%C3%A3o | open:https://wikipedia.org/w/index.php?search=a%C3%A7%C3%A3o | QuickLink.url(for:) (UTF-8 percent encoding) |
| V076 | en | - | - | g c++ & rust | link.g | Google: c++ & rust | https://www.google.com/search?q=c%2B%2B%20%26%20rust | open:https://www.google.com/search?q=c%2B%2B%20%26%20rust | QuickLink.url(for:) (& + = ? # are encoded) |
| V077 | en | - | - | emoji rocket | emoji.🚀 | Rocket | ↩ pastes into the front app | paste:🚀 | EmojiSearch.paletteItems |
| V078 | pt | - | - | emoji coração | @0 | Red Heart | ↩ cola no app da frente | paste:❤️ | EmojiSearch.words, preferred |
| V079 | en | - | - | :fire | emoji.🔥 | Fire | ↩ pastes into the front app | paste:🔥 | EmojiSearch.prefixes (":") |
| V080 | pt | - | - | emoji foguete | emoji.🚀 | Rocket | ↩ cola no app da frente | paste:🚀 | EmojiSearch.words ("foguete") |
| V081 | zh | - | - | emoji 火箭 | emoji.🚀 | Rocket | ↩ 粘贴到前台应用 | paste:🚀 | EmojiSearch.words ("火箭") |
| V082 | en | - | - | emoji rocket | !link.search | * | * | * | Palette.update (emoji prefix hides web search) |
| V083 | en | - | - | arrow | symbol.→ | Seta Direita | ↩ pastes into the front app | paste:→ | EmojiSearch.symbols (title from the first keyword) |
| V084 | en | - | - | euro | symbol.€ | Euro | ↩ pastes into the front app | paste:€ | EmojiSearch.symbols |
| V085 | en | - | - | degree | symbol.° | Grau | ↩ pastes into the front app | paste:° | EmojiSearch.symbols |
| V086 | en | - | - | timer 10m tea | timer.new | 10 min timer: tea | Ends at 9:10 AM | timer:600s:tea | TimerStore.paletteItems |
| V087 | en | - | - | timer 1h30 | timer.new | 1 h 30 min timer | Ends at 10:30 AM | timer:5400s: | TimerStore.duration ("1h30") |
| V088 | es | - | - | temporizador 90s | timer.new | Temporizador de 1 min 30 s | Termina a las 09:01 | timer:90s: | TimerStore.timerWords |
| V089 | pt | - | - | contagem 5 pizza | timer.new | Timer de 5 min: pizza | Termina às 09:05 | timer:300s:pizza | TimerStore.duration ("5" is minutes) |
| V090 | en | - | - | alarm 7:30 meeting | alarm.new | Alarm at Sun 7:30 AM: meeting | Plays a sound and notifies you on time | alarm:meeting | TimerStore.alarmTime (next 7:30) |
| V091 | pt | - | - | alarme 19:00 | alarm.new | Alarme às sáb. 19:00 | Toca um som e avisa na hora | alarm: | TimerStore.alarmTime (today, still ahead) |
| V092 | de | - | - | wecker 7h | alarm.new | Wecker um So. 07:00 | Spielt einen Ton und benachrichtigt pünktlich | alarm: | TimerStore.alarmWords |
| V093 | en | - | - | alarm 7pm | alarm.new | Alarm at Sat 7:00 PM | Plays a sound and notifies you on time | alarm: | TimerStore.alarmTime (pm) |
| V094 | en | - | - | timer 0 | !timer.new | * | * | * | TimerStore.paletteItems (secs > 0) |
| V095 | en | - | - | timer 25h | !timer.new | * | * | * | TimerStore.paletteItems (at most 24 h) |
| V096 | en | - | - | focus 50 | focus.timed | Focus for 50 min | * | focus-start:50 | Palette.parametric |
| V097 | pt | - | - | foco 25 | focus.timed | Foco por 25 min | * | focus-start:25 | Palette.parametric ("foco") |
| V098 | de | - | - | fokus 15 | focus.timed | Fokus für 15 Min. | * | focus-start:15 | Palette.parametric ("fokus") |
| V099 | en | - | - | focus 700 | !focus.timed | * | * | * | Palette.parametric (1...600) |
| V100 | en | - | - | focus | action.focus | Start focus | Type a time, like “focus 50” | focus-start:25 | Palette.commands "focus" (25 min default) |
| V101 | en | - | - | fire | app:firefox | Firefox | * | launch:firefox | PaletteSearch.score (prefix) |
| V102 | en | - | - | mt | app:teams | Microsoft Teams | * | launch:teams | PaletteSearch.score (initials) |
| V103 | en | - | - | clear history | action.clearclip | Clear clipboard history | Deletes every unpinned entry | clear-clipboard | Palette.commands (several words must all appear) |
| V104 | en | - | - | history | @0 | Clipboard history | * | open-clipboard | PaletteModel.update (sorted by score) |
| V105 | en | - | action.clearclip | history | @0 | Clear clipboard history | * | clear-clipboard | PaletteModel.update (recents bonus 60) |
| V106 | en | - | - | (empty) | @0 | Clipboard history | Search and paste what you copied | open-clipboard | PaletteModel.update (suggestions when there are no recents) |
| V107 | en | - | action.uuid,action.focus | (empty) | @0 | Generate UUID | * | generate-uuid | PaletteModel.update (recents first) |
| V108 | en | - | action.uuid,action.focus | (empty) | @1 | Start focus | * | focus-start:25 | PaletteModel.update (recents keep their order) |
| V109 | en | - | action.uuid,action.focus | (empty) | @2 | Clipboard history | * | open-clipboard | PaletteModel.update (suggestions after the recents) |
| V110 | en | - | - | (empty) | @4 | timer 10m tea | Countdown in the tray | fill:timer 10m tea | PaletteModel.update (tips after the 5 first items) |
| V111 | pt | - | - | (empty) | @4 | timer 10m chá | Contagem na bandeja | fill:timer 10m chá | tips in Portuguese (L10n "timer 10m chá") |
| V112 | en | - | - | xyzzy | link.search | Search “xyzzy” on Google | https://www.google.com/search?q=xyzzy | open:https://www.google.com/search?q=xyzzy | QuickLinkStore.webSearchItem is the last item |
