# WorkLog

WorkLog is a personal, local-first work journal for Ubuntu 24.04. It keeps quick notes, timed tasks, and breaks together in a daily timeline. The desktop app uses Tauri 2, Rust, and React. It runs offline and stores the journal in SQLite.

## Features

- **Quick Capture** opens a small entry window from the top-panel menu, an application shortcut, or the dashboard. Press **Enter** to save and **Esc** to hide the window while keeping your draft.
- **Workday tracking** records sessions, tasks, and breaks from the dashboard or tray. After **End day**, the Workday card shows time worked and time spent on breaks.
- **Journal** lets you browse by date, search activities and references, and filter by category. You can edit entries and time ranges or add them after the fact.
- **Daily review** lets you add notes and mark a day reviewed. Editing the day’s records flags it for another look. Review reminders are configurable.
- **Local data controls** include SQLite backup and restore, CSV export, editable categories, reference prefixes such as `NCR`, and light and dark themes.

## Build from source

Use Ubuntu 24.04, Node.js 24 with npm, and the stable Rust toolchain. Install the system dependencies below. [Install Rust](https://rustup.rs/) · [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/)

```bash
sudo apt update
sudo apt install build-essential curl wget file pkg-config libssl-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev librsvg2-dev
```

From the repository root, install the JavaScript dependencies and start the desktop app in a graphical session.

```bash
npm ci
npm run tauri dev
```

### Build and install the Debian package

```bash
npm run tauri build -- --bundles deb
```

Tauri writes the package under `src-tauri/target/release/bundle/deb/`. Build on Ubuntu 24.04 for your target architecture. [Tauri Debian packaging](https://v2.tauri.app/distribute/debian/)

For the 0.2.2 amd64 build, run the following from the repository root. If you are upgrading, choose **Quit WorkLog** from the tray first.

```bash
sudo apt install ./src-tauri/target/release/bundle/deb/WorkLog_0.2.2_amd64.deb
```

Open **WorkLog** from Applications or run `worklog` in a terminal.

## Everyday use

1. Select **Start day**, then **Start task** and enter what you are working on. Starting a task or a break also opens a session if needed.
2. Select **Take break** to finish the current task and begin a break. Use **End break**, **Stop task**, or **End day** to finish tracking.
3. Use the pencil in the Workday card to correct session times, or the trash icon to remove a finished session. Use the timeline’s **+** to add a past activity or break and enable **Set end time** to record a range.
4. Select **Review day** to write a note and mark the day reviewed. Use **Reopen day** when you want to return to it.

**Time worked** totals finished sessions that started on the selected date, subtracting the recorded breaks within them. Overlapping time counts once. Overnight sessions appear on the date they started.

In the dashboard, **Ctrl+N** opens Quick Capture and **Ctrl+F** focuses journal search. Closing the dashboard hides it by default. Change **Minimize to tray** in Settings to use the dock, or select **Quit WorkLog** to stop the app.

## Ubuntu desktop integration

- **Tray** uses the Ubuntu AppIndicators extension. The Applications launcher and `worklog --quick-add` also open WorkLog. The session D-Bus routes launcher actions to the running instance. [Tauri system tray](https://v2.tauri.app/learn/system-tray/) · [Tauri single instance](https://v2.tauri.app/plugin/single-instance/)
- **Wayland shortcuts** can be configured under **Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts**. Set the command to `/usr/bin/worklog --quick-add`. GNOME controls window focus and placement, so click Quick Capture if it opens without keyboard focus. On X11, configure the shortcut in **WorkLog → Settings → Quick Capture**. [Tauri global hotkey platform support](https://github.com/tauri-apps/global-hotkey#platforms-supported)
- **Review reminders** use the Linux notification service. **Open Dashboard** opens the corresponding daily review, and **Snooze** schedules another alert. A restart or resume can deliver a missed reminder up to three hours after its scheduled time on the same date.

## Journal data

The database is at `$XDG_DATA_HOME/io.worklog.desktop/worklog.sqlite3`, typically `~/.local/share/io.worklog.desktop/worklog.sqlite3`. The current location is shown under **Settings → Data**, where you can back up the journal, restore a snapshot, or export entries as CSV. Backups include work sessions and database preferences. WorkLog migrates a backup to the current schema before restoring it.

## Project layout and tests

- `src/` contains the React and TypeScript dashboard and Quick Capture UI, styled with Tailwind CSS.
- `src-tauri/src/` contains the Rust desktop integration and Tauri commands.
- `src-tauri/core/` contains the SQLite store, migrations, workday calculations, and tests.

```bash
npm test
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
```

The test suite covers journal entries, workday totals, reviews, reminders, database migrations, backup and restore, and CSV export.

### GNOME and Wayland smoke test

- [ ] Launch the installed app and confirm that today’s timeline opens. Try Quick Capture from the tray, Applications launcher, and desktop shortcut. Verify that a second launch opens the existing app.
- [ ] Enter `NCR 2456 - Tested the issue` in Quick Capture and press Enter twice. Verify that one entry is saved with its reference. Confirm that an unsaved draft survives **Esc**.
- [ ] Start and stop a task, take a break, and end the day. Check the net work total, then edit the session and verify that the total updates. Try adding a past activity and break.
- [ ] Search and filter the journal, edit categories, and review a day. Confirm that a later edit flags the day for review.
- [ ] Check theme, time format, tray behavior, and launch at login. Send a test notification, then schedule a review reminder to verify its dashboard and snooze actions.
- [ ] Back up the journal, export entries to CSV, and try cancelling and confirming a restore with test data.
