# Bug report triage

This is the process for bug reports after v1 is released. It covers intake, severity, what to ask
for, and when a report is closed. Release steps are in [release.md](release.md).

## Intake

Reports arrive as GitHub issues in the tpt-solutions organisation. Each report gets a first reply
within five working days, even if the reply is only a request for more information.

Security reports do not go in a public issue. Ask the reporter to contact TPT Solutions privately
first, then fix and release before describing the problem publicly. This covers font parsing
crashes that could be used to run code, and anything that lets one user's files be read or changed
by another.

## Asking for the right information

Ask for these, in order. Do not ask for the font file itself: it may be licensed, and the log
usually shows what the parser saw.

1. Version (`tpt-fonts --version` from the CLI, or the installer name), operating system and
   version, and the desktop (Windows, or Linux with X11 or Wayland).
2. Which part was used: the GUI, the CLI, or the tray.
3. The steps to reproduce, and what happened compared with what was expected.
4. The local log. It is at `%LOCALAPPDATA%\tpt-fonts\logs\tpt-fonts.log` on Windows and at
   `$XDG_STATE_HOME/tpt-fonts/logs/tpt-fonts.log` on Linux (`~/.local/state` when that variable is
   unset). Ask for the lines around the time of the problem. The log rotates at 1 MiB into
   `tpt-fonts.log.1` in the same folder, so the entry may be in that file instead.
5. For a font that will not list, install, or preview: the font's family name, format
   (TTF, OTF, TTC or variable), and whether it opens in another application.

Nothing is sent automatically. The log stays on the reporter's machine until they paste it.

## Severity

| Label | Meaning | Target |
|-------|---------|--------|
| `severity:critical` | Data loss (a font file or a user font is deleted or overwritten), a crash on start-up, or a session font left active after the tray has exited | Fix or workaround within a week |
| `severity:high` | A main feature does not work (install, activate, deactivate, uninstall, duplicate resolution) on a supported platform | Next patch release |
| `severity:medium` | A feature works but gives a wrong result or a confusing error | Next minor release |
| `severity:low` | Cosmetic issues, wording, small layout problems | When convenient |

A report that involves the duplicate trash flow is `severity:high` by default until it is shown
to be harmless, because the trash path has only been checked with a dry run.

## Labels

- `platform:windows`, `platform:linux`, `platform:x11`, `platform:wayland`
- `component:gui`, `component:cli`, `component:tray`, `component:core`, `component:installer`
- `kind:bug`, `kind:docs`, `kind:question`, `kind:feature` (feature requests go to the backlog in
  [todo.md](../todo.md) and are not scheduled by triage)
- `needs-info` while waiting for the reporter; the issue is closed after 30 days without a reply

## Reproduction

1. Reproduce on the same platform and version. A report from Linux is not reproduced on Windows.
2. If the report concerns a font file, check the font with the parsing tests and the fuzzing test
   (`core/tests/fuzz_parse.rs`) before concluding the parser is at fault.
3. For session problems, check the state file (`session.json` in the state directory) and whether
   a tray process is running. The tray's `--cleanup` option removes session fonts left behind.
4. Record the result in the issue, including the commands that were run.

A report is closed as **fixed** when a change that addresses it is merged and released, with a
test where practical. It is closed as **not reproduced** after a reasonable attempt and no new
information, or as **by design** when the behaviour is the documented one (for example, session
fonts end when the tray exits).

## Weekly review

Once a week, one person reviews open issues:

- Move `needs-info` issues with no reply for 30 days to closed.
- Check every `severity:critical` and `severity:high` issue has an owner and a next step.
- Check whether any new report repeats a limit listed as open in [todo.md](../todo.md). If it does,
  link them and note the item rather than filing a duplicate.
