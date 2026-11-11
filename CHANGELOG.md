# Changelog

## Unreleased — 0.1.0 experimental

- Linux file manager written in Rust, with native X11 and Wayland windows.
- Folder tabs with distinct outlines, integrated close buttons, drag-to-reorder with edge scrolling, and persistent ordering/active-tab identity.
- Scrollable Recently closed tabs submenu for reopening a selected entry independently from the latest-tab shortcut.
- Reopen closed tabs with Ctrl+Shift+T and context actions, retaining browsing context and neighbor order across restarts, including home-folder startup and explicit destinations.
- Subtle active-tab backgrounds derived from the system accent, without a colored border or stripe.
- Compact touching tabs with an opaque theme-matching strip and a visible tab-shaped new-tab control.
- Tab context menus for duplication, closing other tabs and closing tabs to the right, with configurable icons and protection for ongoing file operations.
- Independent tab navigation, shared clipboard/notifications and system-opening reuse of inactive tabs.
- Automatic persistence of open tab directories, their order and the active tab, with a General setting to restore the last session or always start in the home folder.
- Explicit Open in new window and Open in new tab folder context actions.
- Resizable integrated terminal dock with real PTY sessions, shell input, ANSI output and independent working directories.
- Details, list and icon views, configurable columns, zoom and per-folder layouts.
- Multiple selection, keyboard navigation, copy, move, rename and Trash operations.
- Native drag and drop, system default applications and an Open With chooser.
- Add network share dialog with persistent addresses, native GVfs/GIO authentication on GNOME or KIO on KDE, cancellation and reconnect actions; native Nautilus/KDE Samba folder-sharing configuration from folder context menus.
- Configurable sidebar, pinned folders, mounted network shares and Trash.
- Portuguese and English interfaces, with configurable Font Awesome icons.
- Stacked notifications with per-type appearance, timing and configuration previews.
- Initial native packaging, desktop metadata, license inventory and release preparation.
- FileManager1 integration for Show in Folder, grouped selections and properties, with opt-in user activation and rollback.
- Show in Folder reuses matching windows, refreshes their selection and forwards Wayland activation tokens to request foreground focus.
- Normal system/desktop directory launches reuse matching windows as well, preserving their selection, filter and per-folder layout.
- One application icon shared by native windows, desktop launchers and taskbar entries, with KDE icon-cache refresh.

No public release or store submission has been made yet. This version is experimental.
