# Review feedbacku — ověření + návrh řešení + akceptační kritéria

**Datum:** 2026-07-01
**Stav:** čeká na schválení seniorem (body 2 + 3). Oprava zatím NEZAHÁJENA.
**Postup po schválení:** oprava proběhne formou TDD (RED → GREEN → REFACTOR), jeden nález = jedna série.

Každý nález je ověřen proti aktuálnímu kódu (odkazy `soubor:řádek`). U každého je:
verdikt ověření · kořenová příčina · návrh řešení · akceptační kritéria (testovatelná).

---

## Nález 1 — HIGH · Obnova zálohy selže na FK pro reálná data

**Verdikt: POTVRZENO (a je to horší, než feedback uvádí).**

### Ověření
- FK jsou zapnuté per-connection: `src-tauri/src/cache/db.rs:28` (`PRAGMA foreign_keys = ON`).
- Import běží v jedné transakci a **nevypíná** FK: `src-tauri/src/commands/backup.rs:191-219`.
- Pořadí insertů (`backup.rs:210-212`): `worklogs` → `issues_v2` → `connections`, tedy **potomci před rodiči**.
- FK definice (`src-tauri/migrations/0012_worklogs_v2.sql`):
  - `worklogs.connection_id INTEGER REFERENCES connections(id)` — nullable (`:53`).
  - `issues_v2.connection_id INTEGER NOT NULL REFERENCES connections(id)` — **NOT NULL** (`:99`).
- SQLite kontroluje FK okamžitě (constrainty nejsou `DEFERRABLE`), takže insert potomka před rodičem spadne hned.

### Kořenová příčina
Insert pořadí je obrácené oproti FK směru.
- `issues_v2` má `connection_id` **NOT NULL** → **jakýkoli** řádek issues_v2 vložený před `connections` spadne na FK (netýká se to jen worklogů).
- `worklogs` spadne, jen když má neprázdné `connection_id` (reálná synchronizovaná data).
- Navíc: `favorite_issues`, `daily_activity`, `non_working_days` se vkládají přes `let _ = insert_rows(...)` (`backup.rs:215-217`), tj. **chyba se tiše spolkne**. `favorite_issues` má FK na `connections` (`migrations/0011_favorites.sql:10`) — při současném pořadí by insert favoritů selhal na FK a **oblíbené položky by se po obnově tiše ztratily**.

### Návrh řešení
1. Vkládat v pořadí rodiče → potomci: `connections` NEJDŘÍV, pak `issues_v2` a `worklogs` (mezi sebou nezávislé — `worklogs` nemá FK na `issues_v2`), poté `audit_log`, `app_settings`, a nakonec `favorite_issues`, `daily_activity`, `non_working_days`.
2. Zvážit, zda chyby u `favorite_issues`/`daily_activity`/`non_working_days` opravdu polykat. Minimálně po přeřazení pořadí už FK chyba nenastane; pokud je polknutí záměrné (dopředná/zpětná kompatibilita schématu), doplnit komentář proč.

### Akceptační kritéria
- [ ] AK1.1: Round-trip test (Rust integrační test nad dočasnou DB): vytvoř `connection` + `issue_v2` + `worklog` (worklog s neprázdným `connection_id` a `issue_key`), proveď export → import do čisté DB. Import projde bez chyby a počty řádků v `connections`/`issues_v2`/`worklogs` sedí na originál.
- [ ] AK1.2: Stejný round-trip zachová i jeden `favorite_issues` řádek navázaný na connection (počet favoritů po importu == před exportem).
- [ ] AK1.3: RED-fáze: test z AK1.1 musí na současném kódu **selhat** na FK chybě (potvrzuje reprodukci) předtím, než se přehodí pořadí.
- [ ] AK1.4: Zpětná kompatibilita — import legacy zálohy, kde worklogy mají `connection_id = NULL`, dál funguje.

---

## Nález 2 — MEDIUM/HIGH · Worklog přes půlnoc se v denních součtech přiřadí celý ke startovnímu dni

**Verdikt: POTVRZENO jako chování. Vyžaduje produktové rozhodnutí, než se „opraví".**

### Ověření
- Agregace bere celý worklog podle `started_at`: `src-tauri/src/cache/worklogs.rs:253-267` (`for_date_range` filtruje `w.started_at BETWEEN ?1 AND ?2`).
- Streaky sčítají celou dobu trvání do dne `started_at`: `src-tauri/src/commands/streaks.rs:69-79` (`per_day[date_of(started_at)] += duration_s`).
- UI umí konec po půlnoci (23:30 → 00:30 → 3600 s): test `src/routes/WorklogRow.test.tsx:101-133`. Tento test ale ověřuje **jen výpočet délky ve WorklogRow**, nikoli přiřazení ke dni v agregaci — agregační stranu nepokrývá žádný test.

### Kořenová příčina
Denní agregace bere interval jako atomický a řadí ho do dne začátku. Interval překračující půlnoc se nerozděluje na části podle hranic dnů.

### Návrh řešení
**Nejdřív produktové rozhodnutí** (dvě varianty — nutno zvolit):
- **A) Záměrné pravidlo „počítá se den startu"** — pak neopravovat kód, jen doplnit test, který toto pravidlo pojmenuje a zafixuje (aby se to nezměnilo omylem), a stručně zdokumentovat v komentáři u `for_date_range`/streaks.
- **B) Správně = ořez podle dnů** — při agregaci (reporty i streaky) interval rozdělit na denní části a každému dni přičíst jen jeho průnik. Sjednotit na jednom pomocném místě (jedna funkce „rozřež interval na denní úseky"), ať reporty i streaky používají totéž.

### Akceptační kritéria
> Platí verze pro variantu B; u varianty A se AK2.1/2.2 obrací na „očekává se celý čas ve dni startu".
- [ ] AK2.1: Jednotkový test agregace: worklog 23:30–00:30 (60 min) přispěje 30 min do dne D a 30 min do dne D+1.
- [ ] AK2.2: Streak test: den D má cíl splnitelný jen započtením správného podílu půlnočního worklogu → výsledek `current`/`today_met` odpovídá zvolenému pravidlu.
- [ ] AK2.3: Test hranic — worklog přesně 00:00–00:00 dalšího dne (24 h) se rozdělí korektně bez dvojího započtení ani mezery.
- [ ] AK2.4: Regrese — worklog nepřekračující půlnoc dává identické výsledky jako dnes.

---

## Nález 3 — MEDIUM · Backend nerevaliduje provider URL (frontend komentář tvrdí opak)

**Verdikt: POTVRZENO.**

### Ověření
- Frontend vyžaduje `https://`: `src/lib/validation.ts:19-22` (`urlSchema`). Komentář v hlavičce souboru (`validation.ts:6-7`) explicitně tvrdí: *„The Rust side re-validates so this layer is 'fast feedback, never authoritative'"*.
- Backend `add_connection` validuje jen název, provider, token a (u Jiry) JQL: `src-tauri/src/commands/connections.rs:259-282`. **Žádná validace URL schématu/hosta.**
- Klienti jen parsují URL bez omezení schématu/hosta:
  - Jira: `src-tauri/src/jira/client.rs:82-83` (`Url::parse(&base_url)?`).
  - Freelo: `src-tauri/src/freelo/client.rs:71-77` (`Url::parse(&url)?`).

### Kořenová příčina
Kontrola `https://` žije jen v rendereru (Zod). Backend se spoléhá, že vstup je čistý, ale `Url::parse` přijme `http://`, `ftp://`, `localhost`, privátní IP i libovolný host. Kompromitovaný renderer nebo **importovaná konfigurace ze zálohy** tak může poslat token na cizí host (exfiltrace).

### Návrh řešení
Přidat autoritativní validaci base_url na backendu (sdílená funkce volaná z `add_connection`, `update_connection` i při hydrataci importované konfigurace):
- vyžadovat schéma `https` (mimo dev),
- odmítnout URL s embedded credentials (`user:pass@`),
- odmítnout `localhost`/loopback/privátní a link-local IP rozsahy mimo dev,
- volitelně host allow-list nebo explicitní „custom" režim pro on-prem instance.

### Akceptační kritéria
- [ ] AK3.1: Rust unit testy — `add_connection`/validační funkce **odmítne**: `http://…`, `ftp://…`, `https://user:pass@host`, `http://localhost`, `https://127.0.0.1`, `https://10.0.0.5`, `https://169.254.1.1`.
- [ ] AK3.2: **Přijme** validní `https://firma.atlassian.net` a nastavený custom/on-prem host dle zvoleného pravidla.
- [ ] AK3.3: Import zálohy s connection mířící na nedovolený host je odmítnut (nebo connection označen jako neaktivní) — token se na takový host neodešle.
- [ ] AK3.4: Chování dev vs. produkce je řízeno jasným přepínačem (aby localhost šel v dev testech).
- [ ] AK3.5: Komentář ve `validation.ts` odpovídá realitě (buď backend opravdu revaliduje, nebo se komentář opraví).

---

## Nález 4 — MEDIUM · Favorites a quick-start ztrácí kontext connection

**Verdikt: POTVRZENO.**

### Ověření
- PK je jen `issue_key`: `src-tauri/migrations/0011_favorites.sql:8-9` (`issue_key TEXT PRIMARY KEY`, `connection_id` je jen sloupec).
- `add` dělá `ON CONFLICT(issue_key) DO UPDATE SET connection_id = excluded.connection_id`: `src-tauri/src/cache/favorites.rs:11-21` → oblíbená položka je globálně jedna na `issue_key` a connection se přepíše.
- `list_keys` vrací jen `Vec<String>` klíčů: `favorites.rs:43-50` (connection_id se zahazuje).
- UI dedupuje a vybírá jen podle `issue_key`: `src/components/Layout/StartTrackingBar.tsx:122` (`favoriteKeys = new Set(favorites.map(f => f.issue_key))`), výběr `handlePick(issueKey)` `:148-154`, `onPickIssue(issueKey, c)` `:153`.

### Kořenová příčina
Model favoritů je jednotenantní: identita úkolu = `issue_key`. Při více Jira tenantech se stejným klíčem (např. `PROJ-1` existuje v obou) se favority slévají a UI/quick-start může zobrazit nebo spustit špatný issue.

### Návrh řešení
- Změnit identitu favoritu na `(connection_id, issue_key)` — buď složený PK, nebo `UNIQUE(connection_id, issue_key)` (migrace + `ON CONFLICT` cíl).
- `list`/DTO nesou `connection_id`; UI dedupuje a spouští podle `(connection_id, issue_key)`.
- Migrace stávajících řádků: dosadit `connection_id` (u NULL rozumný fallback / ponechat jako „neurčeno").

### Akceptační kritéria
- [ ] AK4.1: Rust test — přidání `PROJ-1` pro connection A a `PROJ-1` pro connection B vytvoří **dva** různé favority; `list` je vrátí oba s příslušným `connection_id`.
- [ ] AK4.2: `remove` odstraní jen favorita daného `(connection_id, issue_key)`, druhý zůstane.
- [ ] AK4.3: FE test — quick-start z favorita předá `connection_id` a spustí správný issue v rámci správné connection.
- [ ] AK4.4: Migrace existující DB proběhne bez ztráty řádků; RED test na kolizi klíčů před opravou selže.

---

## Nález 5 — MEDIUM · Undo smazání worklogu hledá jen podle remote ID

**Verdikt: POTVRZENO.**

### Ověření
- FE volá undo jen s remote id: `src/routes/TimeLog.tsx:258` (`await undoDeleteWorklog(jiraId)`).
- Backend command bere `worklog_id: String` a hledá globálně přes remote_id: `src-tauri/src/commands/worklog/crud.rs:1007-1014` → `get_pending_delete_by_remote_id_any`.
- Lookup: `WHERE remote_id = ?1 AND pending_delete_at IS NOT NULL … ORDER BY pending_delete_at DESC LIMIT 1`: `src-tauri/src/cache/worklogs.rs:366-384`. Doc komentář `:362-363` sám přiznává, že `remote_id` je unikátní **jen** v rámci `(connection_id, remote_id)`.

### Kořenová příčina
Undo se spoléhá na předpoklad „v okně je právě jeden pending-delete se stejným remote_id". Ten padá, když jsou dvě connection s **kolizí remote_id** (Jira tenanti číslují worklog ID nezávisle) a uživatel smaže dva takové worklogy rychle po sobě uvnitř 5s undo okna — `LIMIT 1 ORDER BY pending_delete_at DESC` pak může vrátit špatný řádek. (Sekundárně: pro čistě lokální worklogy bez `remote_id` je lookup podle remote_id nespolehlivý.)

### Návrh řešení
Předat undo **lokální row `id`** (příp. `(connection_id, remote_id)`) místo pouhého remote_id:
- FE si drží lokální `id` mazaného řádku a pošle ho do `undo_delete_worklog`.
- Backend hledá pending-delete podle lokálního `id` (jednoznačné). Zpětně tolerovat starý parametr, dokud FE nepřejde.

### Akceptační kritéria
- [ ] AK5.1: Rust test — dvě connection, dva worklogy se **stejným** `remote_id`, oba pending-delete; undo cílené na řádek A obnoví přesně A (ne B).
- [ ] AK5.2: Undo lokálního worklogu bez `remote_id` funguje (dohledá se podle lokálního id).
- [ ] AK5.3: FE test — undo toast pošle lokální id smazaného řádku.
- [ ] AK5.4: RED test z AK5.1 na současném kódu selže (vrátí špatný řádek), po opravě projde.

---

## Souhrn priorit

| # | Severita | Verdikt | Bloker pro opravu |
|---|----------|---------|-------------------|
| 1 | HIGH | Potvrzeno (horší než hlášeno) | ne — začít tímto |
| 2 | MED/HIGH | Potvrzeno chování | **ANO — potřeba produktové rozhodnutí A vs. B** |
| 3 | MEDIUM | Potvrzeno | ne |
| 4 | MEDIUM | Potvrzeno | migrace DB — potřeba odsouhlasit fallback pro NULL connection_id |
| 5 | MEDIUM | Potvrzeno | ne |

**Otázky na seniora (zodpovězeno):**
1. Nález 2 → **varianta B** (ořez podle dnů).
2. Nález 3 → https + zákaz credentials/private IP + SaaS allow-list + custom režim (opt-in), dev jen přes `cfg(debug_assertions)`.
3. Nález 4 → migrace doplní connection_id u jednoznačných klíčů, víceznačné nechá NULL (skryté).

---

## Stav implementace (2026-07-01) — vše hotovo TDD, testy zelené

Gate: backend `cargo test` **211 passed**, FE `vitest` **272 passed**, `tsc --noEmit` čistý, `cargo fmt --check` + `cargo clippy` čisté.

| # | Stav | Klíčové změny | Nové testy (RED→GREEN) |
|---|------|---------------|------------------------|
| 1 | ✅ | Přeřazeno insert pořadí v `import_inner` (rodiče první); opraven export `favorite_issues.added_at` (dřív `created_at` → favority se neexportovaly) | `restore_roundtrips_connection_issue_worklog`, `restore_preserves_favorites_with_connection`, `restore_legacy_null_connection_worklog` |
| 4 | ✅ | Migrace `0015` (favorites PK `(connection_id, issue_key)` + backfill), `0016` (active_timer.connection_id). Cache/commands nesou connection_id; timer pipeline (start/stop/auto-transition + record_local_stop) preferuje `timer.connection_id`. FE: FavoriteStar/StartTrackingBar dedup + pick nesou connection_id, `startTimer(connectionId)` | `same_key_two_connections_yields_two_favorites`, `remove_is_scoped_to_connection`, `list_skips_ambiguous_legacy_favorite`, `stop_attributes_worklog_to_timer_connection`, FE `threads connectionId…` |
| 5 | ✅ | `undo_delete_worklog` bere lokální row id (extrahován `undo_delete_worklog_inner`), FE posílá `row.id` | `undo_restores_exact_local_row_despite_remote_id_collision` |
| 3 | ✅ | `validation::validate_base_url_safety` + `validate_provider_base_url` (https, žádné credentials, žádná private/loopback/link-local/multicast IP, allow-list `*.atlassian.net`/`freelo.io` + Jira custom přes `allow_custom_host`). Voláno v add/update_connection + defense-in-depth v konstruktorech klientů. Komentář ve `validation.ts` opraven | `base_url_safety_rejects_dangerous_targets`, `base_url_safety_accepts_public_https`, `provider_url_enforces_host_allowlist` |
| 2 | ✅ | Sdílený clip primitiv `day_overlap_seconds` + `overlap_seconds_for_range` (SQL). Streaks distribuují worklog přes dny; daily-goal notifikace přes overlap. FE: `lib/dates.dayOverlapSeconds` + Goals `todaySeconds` ořezaný | `day_overlap_clips_cross_midnight_worklog`, `overlap_seconds_for_range_clips_each_worklog`, FE `dayOverlapSeconds` (4×) |

### Review round 2 — opraveno po seniorském nálezu

Gate teď: **`cargo test` (všechny targety, ne jen `--lib`) zelený**, FE 272, `tsc`/`fmt`/`clippy` čisté.

1. **BLOCKER — integrační testy nekompilovaly.** Doplněn 5. arg `None` do `start_timer_inner` call-sites (`tests/server.rs`, `tests/commands.rs`), `connection_id: None` do ručních `ActiveTimer`, aktualizován migrační seznam v `tests/cache_db.rs` na `1..=16`, a rozšířena assertion `test_jira_connection_inner_rejects_bogus_url` (bogus URL teď padá jako `InsecureUrl` z safety pre-checku).
2. **#3 díra na import/hydrataci uzavřena.** `validate_provider_base_url` se nově enforce i v `state.rs` hydrataci (při chybě connection přeskočí → token neodejde), v `test_connection_for_provider` a v legacy `test_jira_connection_inner`. Přidán dev-loopback bypass (`cfg!(debug_assertions)` + loopback/localhost), takže mock testy fungují, ale `https://evil.com` v release je odmítnut.
3. **#4 popover tenant.** `startForIssue`/`RecentList` nesou `connection_id`, `startTimer(connectionId)`, klíč `(connection_id, issue_key)`.
4. **#2 zbývající denní povrchy.** Root fix: `for_date_range` je nově **overlap** (vrací worklogy překrývající okno, ne jen `started_at BETWEEN`) — přetékající řádky už nechybí. Clip doplněn: popover denní cíl + live timer, `DailyBarChart` (split přes dny), `Calendar` (`totalsByDay` split + month/year period-clip), `Reports` (`totalSeconds`/`aggregateByIssue`/`uniqueDays`), `Goals` (`monthSeconds`).

**Poznámka k chování (k vědomí):** změna `for_date_range` na overlap znamená, že worklog přetékající půlnoc se v Time Logu objeví v obou dnech, kterých se dotýká (dřív jen ve dni startu). Je to konzistentní s variantou B; pokud senior chce v seznamu jen den startu, oddělíme list-fetch od total-fetch.

### Review round 3 — opraveno

1. **MED/HIGH — legacy Jira config obcházel allow-list.** `state.try_build_client` (jediný build point legacy shimu, který krmí `jira_client_cloned()` → sync/issues) nově volá `validate_provider_base_url("jira", base_url, false)` před `JiraClient::new`. Navíc early validace v `save_config` i `update_config_inner` (reject před zápisem config.toml). `https://evil.com` tak neprojde ani legacy IPC.
2. **LOW/MED — okrajové nekonzistence overlap modelu.** `for_date_range` je nově half-open v `from` (`w.ended_at > ?1 AND w.started_at <= ?2`) → worklog končící přesně v půlnoci se v dalším dni neobjeví jako phantom nula (test doplněn). `Reports.uniqueDays` počítá dny přes `dayOverlapSeconds > 0` per den v rámci periody, ne podle `started_at` (přetékající řádky se započtou do správných in-period dnů).

Gate: `cargo test` (všechny targety) zelený, FE 272, `tsc`/`fmt`/`clippy` čisté.

### Vědomé rozsahové hranice (k potvrzení seniorem)
- **#4b Freelo stop-routing**: Jira POST + lokální atribuce worklogu už jdou přes `timer.connection_id`. Freelo `resolve_*_for_issue` zůstává key-based — Freelo syntetické klíče (`FREELO-*`) nekolidují mezi účty jako Jira project keys. Pokud chceme i zde tvrdou tenant-jistotu, je to malý navazující krok.
- **#3 custom host**: backend enforcement přes config flag `allow_custom_host`; **FE přepínač zatím není** — self-hosted Jira lze zatím nastavit jen importem/konfigem s tím flagem. UI toggle = navazující UX práce.
- **#2 reporty**: opraveny autoritativní denní povrchy (streaks, daily-goal, Goals „dnes"). `Reports.tsx` jsou period/project totály (ne per-day buckety), takže cross-midnight je tam okrajový; `uniqueDays` počítá dny podle `started_at` (kosmetické). Pokud přibude per-day graf, použije `dayOverlapSeconds`.
