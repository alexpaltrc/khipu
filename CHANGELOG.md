# Changelog

All notable changes to Khipu, called Espanso Manager until 0.0.5. Dates are the day the work was finished, not the day it
was published.

## Unreleased

### Added

- **The welcome screen helps when the trial does not work.** If what is in the trial box is
  not on its way to the trigger and a moment passes, a quiet line under it says to clear the box
  and type the trigger in one go: correcting a typo half way through can make Khipu lose track of
  it. The tip at the bottom of that step now points to the launcher, the way out for any trigger
  that is forgotten or does not fire.
- **Khipu has a button on the taskbar.** While its window is open, Khipu shows on the taskbar
  like any other app, so you can switch to it, minimize it and find it again. Closing the window
  still leaves it working in the system tray, without a button. Starting with Windows, it stays in
  the tray, and no button flashes on the taskbar at login.
- **Expansions can have a name.** The edit screen has a new **Name (optional)** field. In the
  list and in the launcher, the name leads the row, in stronger type, before the text. The
  triggers stay lined up in their own column. The detail shows it under the trigger. Search in
  the list and in the launcher finds the name too. It is saved in `base.yml` as espanso's own
  `label`, and travels with export and import. An expansion without a name looks as it did. If
  you had already written a `label` into `base.yml` by hand, editing that expansion no longer
  erases it.

### Changed

- **New installs start with Windows.** An expander that is not running does nothing, so the first
  run turns on **Start Khipu with Windows** by itself. The last step of the welcome screen no
  longer asks: it explains that closing Khipu leaves it in the system tray, among the hidden
  icons, shows how to drag it onto the taskbar, and says how to quit for good: right-click the
  icon and choose **Quit**. Its tip says where to turn starting with Windows off. **Nothing changes for anyone who already uses Khipu**: your choice stays as it is.

- **New installs use `;` as the prefix, not `:`.** Outlook, Teams, WhatsApp, Discord and Slack
  read a colon followed by a word as the start of an emoji, and open their emoji picker over
  `:hi` before it can expand. A semicolon followed by a letter means nothing to any of them. The
  three starter expansions are now `;hi`, `;date` and `;today`. **Nothing changes for anyone
  who already uses Khipu**: your expansions and your prefix stay as they are. To move all of them
  to `;`, pick it in Settings under **Prefix for new expansions**, then press **Apply the current
  prefix to my existing expansions**.
- **Tips names the two halves of an expansion.** The example at the top of Tips now labels
  what you type (the trigger) and what appears (the text), says that together they make an
  expansion, and uses your own prefix. It is outside the sections, so nobody has to open
  anything to read it.
- **The welcome screen names the two halves too.** In the typing demonstration, a small
  **Trigger** label sits above what is being typed and turns into **Text** when it becomes the
  full text. With Windows' animation effects off, the still list of examples has the same two
  words as column heads.
- **A round button for new expansions.** The **+** moved from the top right to a round button in
  your Windows accent colour, at the bottom of the main window, in the middle. It rises into
  place with a small bounce each time the window opens, and the list scrolls a little further so
  the last expansion never hides under it. With Windows' animation effects off, it is simply
  there.
- **The tray picture looks like your taskbar.** The welcome screen's animation and the picture in
  Tips now draw Windows 11's own taskbar: Start, the network and volume icons, and the clock with
  the time it is, in your format. The hidden icons open right above the arrow, the way they do on
  screen, and the pointer clicks the arrow before it carries the Khipu icon down.
- **Khipu speaks for itself.** Tips, the welcome screen, the tray tooltip, the status dot and the
  error messages used to say "Espanso" where they meant Khipu or your expansions. They now say
  Khipu, so nobody wonders which app they are using. The welcome screen's trial expansion is
  now `;khipu`, and it greets you in Khipu's language: «¡Hola!» in Spanish, not "Hi there!".
  Espanso is still named where it is due: the credits, the version line in Settings, the
  licences, and the two rare messages about Espanso's own icon showing up.

### Fixed

- **Edit is always reachable in an expansion's detail.** On a short window, the detail beside
  the list could run past the bottom edge and hide **Edit** and the **⋯** menu. Now its text box
  gives up lines first, down to three whole ones, and if that is still not enough, the detail
  scrolls.

## 0.0.6 — 2026-10-06

### Added

- **Khipu is in the Microsoft Store.** Get it from
  [apps.microsoft.com/detail/9NXXV15RJ94T](https://apps.microsoft.com/detail/9NXXV15RJ94T):
  signed by Microsoft, so it installs without an administrator password or the "Windows protected
  your PC" warning, and it updates itself. The portable ZIP stays, unchanged. The two keep their
  expansions and settings apart; Export in one and Import in the other to move them.
- **A hidden theme.** Somewhere in Tips, a few arrow keys pressed in the right order unlock
  «I got chiiills», a fourth appearance in Khipu's own violet, title bar included. Once found,
  Settings opens by itself with the new option outlined and pointed out for a few seconds; it
  stays in Appearance from then on, and wearing it is up to you. Going back to another theme
  gives the title bar back to Windows. If the option is ever lost, the same keys find it again.
  A copy of Khipu from before this version reads the setting as the light theme.

## 0.0.5 — 2026-10-05

### Added

- **Expansions can be switched off without deleting them.** Each expansion's detail has a
  Status switch: off, it no longer expands and leaves the launcher, but it stays in the list,
  dimmer and at the end of its folder, to copy its text from or turn back on — handy for texts
  you reuse now and then that need no trigger. Several can be switched at once from the
  selection bar, and a whole folder from its options. Turned back on, they expand and show in
  the launcher again. Switched-off expansions are kept in `_disabled.yml`, beside `base.yml`,
  a file Espanso does not load; exports carry the switch, so they arrive off on the next machine
  too.
- **One backup per save, even with expansions switched off.** Each save leaves one folder in
  `backups`, holding the whole library as it was — `base.yml` and, when there is one,
  `_disabled.yml` — instead of a loose copy of each file. To go back, copy both into the match
  folder. The last 20 saves are kept, so switching things on and off no longer halves how far
  back you can go; loose copies from earlier versions age out under the same limit.
- **An installed edition, for the Microsoft Store.** Besides the portable ZIP, the app now
  builds as an MSIX package that Windows installs, updates and removes like any other app. It
  keeps your expansions and settings in Windows' own folder for the app instead of beside the
  program, so updates never touch them; uninstalling removes them, so export first. Start with
  Windows works through Windows' startup list: if it was switched off in Settings › Apps ›
  Startup, the app says so and points there. The portable edition is unchanged.
- **Licence notices for everything inside.** Both editions now carry `THIRD-PARTY-NOTICES.txt`
  beside `LICENSE.txt`: the notice of each library built into Khipu, the Microsoft runtime files
  that come with Espanso, and where to get the source code of Espanso and of Khipu.

### Changed

- **Espanso Manager is now Khipu.** The name is Quechua, for the knotted cords the Incas kept
  their records in. It is the new name everywhere: the window, Start, the tray, the program
  file, `Khipu.exe`, and the ZIP and the folder inside it. A new copy does not take over an old
  one's folder, so to bring your expansions along: export them from the old copy (Settings ›
  Export and import), close it and delete its folder, download the new ZIP and unzip it, and
  import them there. Files exported before the rename import as they always did. Start with
  Windows has to be switched on again in the new copy.
- **The welcome screen no longer promises "no installation"**, now that there is an edition
  that installs. It says no account instead.
- **A new icon.** The keycap gives way to a small bird on a knotted cord, the same everywhere:
  the program file, Start and its search, the taskbar, the window, the tray and the welcome
  screen. In the tray it follows the taskbar, dark or light, and paused it keeps the same
  shape, only dimmer. It is drawn at the exact size each screen asks for, so it stays sharp at
  any display scale, and small sizes get bolder strokes so the bird still reads at 16 pixels.

## 0.0.4 — 2026-10-04

### Added

- **A launcher of its own for expansions, on Alt+Space, from any application.** It searches the
  library and fires the expansion you pick, without opening the main window. It opens over
  whatever is in front, even with the app tucked away in the tray, as a small bar with the
  library's list under it: type to narrow the list, arrows and Enter to pick, Esc or a click
  elsewhere to close. The expansion lands where the cursor was, whole and at once, and words in
  front of it are no longer eaten. Espanso's own search bar is switched off, since only one of the
  two can have the shortcut.
- **The launcher's shortcut is chosen in Settings.** Click the box and press it, the way a game
  takes a key: a key with Ctrl, Alt or Win, an F key or another key that stands alone, or the
  middle or side buttons of the mouse. Esc or a click elsewhere leaves the old one in place, a
  shortcut another program already holds is turned down with a line saying so, and a button
  beside it goes back to Alt+Space.

### Changed

- **The first run is a new welcome, four short steps instead of one long page.** The app's mark,
  name and version; then what an expansion is, shown rather than told — `:hola` turning into a
  greeting, `:fecha` into today's date, `:firma` into a signature at the foot of an email, typed
  out and replaced one after the other; then trying `:espanso` for real in a box; then whether to
  start with Windows, with the pointer acting out how to bring the icon out from behind the
  tray's little arrow. Under each step, one small note: that it is free, private and needs neither
  an installation nor the internet, that it is light enough for the computer not to notice it,
  that expansions can be paused from the tray, and that starting with Windows can be changed later
  in Settings. Next and Back stay in the same place on every step, with dots for where you are,
  and each step slides in. With Windows' animation effects off, nothing moves: the examples and
  the tray are shown still.
- **The first run speaks Windows' language** — Spanish, Filipino or Hindi when Windows is in one
  of them, English otherwise. It used to be English for everyone until the language was changed
  in Settings.
- **The name is written Espanso Manager, with a space**, everywhere it is shown: the window
  title, the welcome, the messages, the start-up switch and the program's description in the Task
  Manager, and also the file itself, `Espanso Manager.exe`, the ZIP and the folder inside it. It
  is the same app; only the two words were separated. A computer that already started the app
  with Windows goes on doing so: the first time the new file opens in the same folder, it moves
  that entry to the new name and the new file. The recommended way to update, though, is a fresh
  copy: export everything from the old one, unzip the new one and import it there.
- **The tip on searching from any application names the shortcut in the app's language**:
  «Alt+Espacio» in Spanish, the way Windows writes it, instead of «Alt+Space» in every language.
- **The quick guide is in a new order, and says more.** Searching from any application comes
  second, right after finding and editing, and says how to pick (arrows and Enter, or a click),
  that Esc closes it and where the shortcut is changed; the table of keys moves to the end. A new
  topic shows how to build a lookup list: give a set of triggers the same first letters, such as
  `:dddparacetamol`, and typing `ddd` in either search lists them all. The Settings topic covers
  the launcher's shortcut too. Inside a topic, the name of each part is now in Semibold with room
  above it, so it reads as a heading rather than as one more sentence.
- **What you type to fire an expansion is called a trigger everywhere** («activador» in Spanish).
  The guide, the welcome, the editor and the folder options called it a shortcut, the same word
  as for keyboard shortcuts.
- **Espanso is 2.4.1**, the latest, up from 2.4.0. Its one fix is in how a match file is read:
  a value written without quotes inside `[ ]` or `{ }` no longer trips it up.

### Fixed

- **The guide says Backspace undoes an expansion that fired by mistake.** It said Esc, which does
  nothing of the kind; it is Backspace, pressed right after.
- **A text box clicked in the app always shows that it is selected.** Now and then the boxes
  took the typing but showed neither their highlight nor the blinking line, and clicking again did
  not bring them back; hiding the window to the tray and opening it again did. The app took its
  own word for whether the window had the keyboard, and one notice from Windows lost along the way
  left it sure that it had not. It now asks Windows on every frame.
- **The search box is as wide as it was meant to be.** It came out 44 points narrower than the
  room set aside for it, which left it and the folder picker beside it off-centre under the
  title.

## 0.0.3 — 2026-09-30

### Changed

- **Moving the pointer over the list, and scrolling it, costs about a third of the processor it
  did.** Each frame used to wait for the monitor inside the graphics driver, which on NVIDIA
  keeps a core busy the whole time it waits. It now waits for Windows' compositor instead, which
  costs nothing while waiting. Measured on a 200 Hz monitor: 70–100 % of a core down to 32–43 %
  while hovering, 111–114 % down to 33–42 % while scrolling, with the same 200 frames a second.
- **With the app in a different theme from Windows', hovering costs half of what it did again.**
  Light over a dark Windows, or dark over a light one, made the app repaint its title bar's
  colour on every frame, because the window never reports the colour it was given. It now reads
  the colour the bar is actually wearing, which costs next to nothing, and repaints it only when
  Windows has put its own back, as it does when one of its settings changes. Measured over 2,000
  expansions: 38–46 % of a core down to 17–25 % while hovering, with the bar then still repainted
  about twenty times in every thousand frames; reading its colour first took that to none.
  Nothing changes for the app set to follow Windows.
- **Every frame does a third less work, and the app takes 8 MB less memory.** Before drawing,
  the interface toolkit handed the graphics context back to the driver and took it again, twice a
  frame, on Windows only. It now skips that when the context is already in place. Measured over
  2,000 expansions following Windows' theme: 19–24 % of a core down to 13–18 % while hovering,
  22–26 % down to 13–14 % while scrolling, and 85–89 MB of the app's own memory down to 79–80.
- **Hovering over a long list, and scrolling it, takes an eighth less of each frame.** Rows far
  off screen were each handed in turn to the code that draws a row, only to be told to leave a
  gap; whole runs of them are now passed over in one step, landing exactly where the row-by-row
  steps did. Frames with no key pressed no longer go through the list's keyboard, and the folder
  menu prepares its lines only while it is open. Measured over 2,000 expansions, in the time each
  frame spends building the window: 220 µs down to 191 while hovering, 245 µs down to 213 while
  scrolling. The list comes out the same, pixel for pixel.
- **The window opens sooner, and a third sooner when espanso was not running.** Finding espanso
  running, or starting it, used to come before the window was made; the two now happen at once,
  and the status beside the title is still what espanso answered. Measured to the window being on
  screen: 317–348 ms down to 271–301 with espanso running, 694–711 ms down to 504–518 with it
  stopped.
- **Text seen for the first time is drawn in a quarter of the time**, which halves the first
  frame. The interface toolkit prepares each letter's shape the first time it appears, and a
  mismatch in how it recorded the font's weight made it redo a slow step for nearly every letter
  of the Regular weight and of the triggers' face. The letters come out exactly as before, pixel
  for pixel. Measured at start-up: 31 ms of drawing letters down to 7.5, and the window on screen
  in 252–254 ms instead of 271–301 (473–486 instead of 504–518 with espanso stopped).
- **Saving no longer freezes the window.** Espanso is restarted after every save so it picks up
  the change, and the window used to stop until it was back: half a second at best, up to six
  when it is slow, right as the list scrolls to the saved row. The window now says what was
  saved at once and carries on; only a restart that fails comes back to say so, under what was
  saved. Pause and Quit wait for a restart under way, so neither can reach an espanso that is
  being replaced. Measured against the real espanso: each save froze the window for 764–797 ms
  in 0.0.2, and for 5–64 ms now.
- Applying a folder's prefix to its expansions reports a failed restart under the count of
  what was changed, as every other save does, instead of in its place.
- **Every question to espanso is answered in half the time**: start-up, pause and resume, and
  the check after a restart. The app looked for espanso's answer every 30 ms and, because
  Windows rounds that up, usually saw it on the second look. It is now woken by the answer
  itself: a status check went from 67 ms to 37–39 ms, measured against the real espanso.
- **Opening a long list no longer takes 75 MB for a moment.** The list only lays out the rows
  near the screen, but on the first frame of every start it laid out all of them, because it
  waited a frame to use the height it had just measured. With 2,000 expansions, the peak at
  start-up went from 140 MB to about 90.
- **The fonts no longer take memory of the app's own.** Each face used to be copied whole into
  the app; it is now read straight from the Windows file, whose pages are shared with every
  other program drawing in Segoe UI. 5 MB less of the app's own memory, 10 MB in Hindi.

### Fixed

- **The detail beside the list shows the text with its line breaks**, and its copy button copies
  them. Both used the list's one-line preview, so a three-line signature read, and was copied, as
  one line.
- **Starting with Windows starts hidden in the tray**, as it was always meant to. The start-up
  entry asked for it, but the window was being shown anyway right after it first drew itself,
  so it opened at every login. Now only the tray icon appears, and a click on it opens the
  window.

## 0.0.2 — 2026-09-25

What changed since 0.0.1. Designs that were tried along the way and replaced before publishing
are not listed.

### Added

- **Pick the folders when exporting and importing.** Export asks which folders to include;
  import reads the file first and offers the folders that file actually turned out to hold. The
  expansions that belong to no folder are a choice of their own, not a leftover. The question is
  skipped when there is only one folder, because a question with one answer is not a question.
- **Deleting a folder now asks first**, in the same style as the expansion warning, and the
  folder's page says beside the button that its expansions go with it.
- **An inspector beside the list.** Choosing a row shows its whole trigger, its text (wrapped,
  scrollable, with a copy button), its folder, **Edit** and a `…` menu to move or delete it. On
  a narrow window it takes the place of the list, with **Back**.
- **Choosing several expansions** is an explicit mode, entered from the `…` menu, or with Ctrl-
  or Shift-click on a row. Changing folder or search clears the selection and says so.
- **Keyboard shortcuts.** Ctrl+N new, Ctrl+L or Ctrl+F search, arrows and Enter in the list, F2
  folder options, Del delete (always asked), Ctrl+S save, Esc backs out one layer at a time.
  None of them act while text is being typed. They are shown in tooltips, menus and the guide.
- **The editor asks before throwing away unsaved changes**, and only when there is something to
  lose.
- **EspansoManager has its own icon**, in Explorer, on the window and in the notification area.
  The executable used to have none, and the tray borrowed Espanso's. The tray icon follows the light or dark colour of the taskbar,
  and paused is a different shape, not only a different colour.
- **Contrast themes are followed.** With a Windows contrast theme on, the app is drawn in the
  colours chosen for it in Windows' settings.
- **Emoji are drawn with the font Windows itself uses** for them, instead of the small one
  bundled with the interface toolkit. `👍` and `💡` used to be an illegible tangle and a bare
  ring; now they read as what they are. They remain single-colour: the toolkit this app draws
  with paints every glyph in one flat colour, so colour emoji are not possible, and the Windows
  colour emoji font would have added 12 MB to the download without changing a single pixel.
  The app's own small icons are deliberately not drawn from it, so no button changed shape.

### Changed

- **A new, quieter interface.** One library with nothing permanently docked beside it:
  - A centred title, with the real state of Espanso on its left — a dot and **Active**,
    **Paused** or **Not responding**, which is what Espanso answered, not what the app assumes —
    and three borderless commands on its right: pause/resume, new, and a `…` menu.
  - Under it, a short centred search and an **All folders** menu that picks which folder is
    shown.
  - Rows are the trigger in a monospaced face and a one-line preview, split by faint lines.
  - The editor, the folder page, the guide and the welcome screen have no cards, numbers or
    subtitles: headings and space do the grouping. The editor's **Save / Cancel** is always on
    screen, and a text draft survives switching to a date and back.
  - Saving and coming back keeps the folder, the search and the scroll position. The saved row
    scrolls into view and is briefly tinted.
- **Settings is a dialog.** It opens in the middle of the window over the dimmed list and cannot
  be moved; Esc, the cross or a click outside it closes it. It names its five settings and
  nothing else: the explanations moved to the guide, under a new **Settings** topic, and so did
  the credit to Espanso's author. What export and import report is shown inside the dialog.
  Turning start-with-Windows on or off no longer posts a banner (the switch already says so); a
  failure still does. On a short window the dialog scrolls under a fixed title.
- **The colours are Windows 11's own**, in light and in dark, and the accent is the one chosen in
  Windows' settings, in the shade Windows itself uses for that theme.
- **The title bar uses Mica**, the Windows 11 material that takes a faint tint from the desktop
  wallpaper. Windows only draws it while the window is active, and a solid-colour desktop gives it
  nothing to tint with. Versions of Windows older than 11 ignore it.
- **Fluid hover.** One soft highlight glides from row to row under the pointer instead of each
  row lighting up and going dark on its own. The same glide runs through the command bar, every
  menu, the switches in Settings and the topics of the guide. Outlined buttons keep their own
  hover, because their fill is part of their shape.
- **Windows' animation effects switch applies to the whole app, live.** With it off, nothing
  travels, slides or fades, and the change takes effect without restarting. The mouse wheel
  keeps its short smoothing: Windows keeps that under a separate setting of its own.
- **Every dialog is the same dialog.** Move, new folder, delete confirmations and the export and
  import folder pickers share one frame, one margin and one button order: action first, then
  Cancel, as Windows does it. Their width shrinks with the window. The pickers are a list of
  folders with a check; a new folder is made with **Create**, not **Save**.
- **The warning shown before deleting expansions is drawn inside the app.** It used to be a
  system dialog, which meant it always looked light even in dark theme and matched nothing else
  on screen. It now follows the theme, lists the expansions **in the same order as the list
  behind it**, folds a long selection into a "Show N more" line instead of running off the edge
  of the window, and hands keyboard focus back where it came from when it closes. There is no
  warning triangle beside the heading, on purpose: Windows 11 does not use one in its own
  dialogs, and the red sentence and the red button already say what kind of question this is.
  Its **Delete** button is legible in light theme, and text being typed into a form survives
  cancelling it.
- **Keyboard focus is visible.** Tab draws a ring around the focused control in both themes.
  Before, only text boxes showed where the keyboard was.
- **Counts agree with their nouns** in all four languages ("1 expansion", not "1 expansions").
- The smallest window is now 620×540 (it was 620×480).
- The start-up switch reads **Start EspansoManager with Windows**, without "(and Espanso)".

### Fixed

- **Emptying a folder no longer deletes it.** A folder that existed only because of what was in
  it vanished when the last expansion was moved out. Folders like this came from an import or
  from typing a new name in the editor. It now stays, empty, like one made with **New folder**.
- **Import no longer counts duplicates as additions.** The folder picker offered "2 will be
  added" when one of them already existed. It now counts only what will really be added. It
  hides folders that would add nothing, and reports what was skipped.
- **A failed restart no longer hides the import result.** The Espanso error used to replace
  "Added: N". Now the result comes first and the error below it.
- **Saving to a read-only `base.yml` keeps what you typed.** The change used to stay in memory
  though not on disk, and the draft was lost. Now the editor stays open with the draft intact,
  and the import is undone in the same way.
- **Hindi buttons were empty boxes.** The Devanagari face was missing from the font family that
  button labels use. Pause, New expansion, Settings and the rest were illegible.
- **Text ran off the right edge** of the welcome screen and the quick guide in narrow windows.
- **"Back" and the other link-style buttons that start a line** have their first letter on the
  page's left edge, like every other line. Their invisible padding put them 14 px further in.
- **A trigger made only of symbols can be created.** `:--`, `:_` and `:—` were refused with
  "the trigger cannot be empty", which was not what had happened: the word box was being read
  as a second prefix, which left no word at all. A word box holding nothing but symbols is now
  a word. The same rule reached two other places — "apply the current prefix to my existing
  expansions" would have rewritten `:--` to just `:`, and one such trigger in a folder used to
  refuse that whole folder's prefix change; both now leave it exactly as it is.
- **"Start with Windows" tells the truth about whether it will.** The switch only checked that
  a start-up entry with our name existed. An entry naming a copy of the app in a folder that
  has since been moved or renamed starts nothing, and an entry Windows itself has been switched
  off — in Task Manager's *Startup apps*, in Settings, or by any of the tidy-up utilities people
  run — starts nothing either, because Windows records that decision in a second place and does
  not delete the entry. Both read as on. The switch now checks that the registered command is
  *this* executable and that Windows has not vetoed it, and turning it on repairs both: it
  rewrites the path and withdraws the veto.
- **Text boxes losing focus on their own.** Clicking the search box, or a box in the
  new-expansion form, sometimes left it unable to accept a single character. Windows had left
  the thread with no focus window at all while the window itself stayed active, and the app now
  notices and puts focus back.

## 0.0.1 — 2026-09-06

First public release.
