# Changelog

## Unreleased

- Add asynchronous icon-view thumbnails for raster images, SVG and first PDF pages, with bounded decoding/cache, metadata invalidation, cancellation and file-type fallback.
- Show logical and allocated disk sizes in Properties, including background recursive folder totals, hard-link deduplication, no-follow traversal, partial results and close-to-cancel behavior.

## 0.1.0-alpha.1 — 2026-11-12

- First GitHub alpha with Ubuntu 24.04 x86_64 DEB/TAR.GZ packages, SHA-256 checksums, build/audit/license records and bilingual installation notes.
- Include unmodified MPL dependency source and font/icon attribution in each binary package; preserve Debian prerelease ordering and validate install/reinstall/removal in a clean container.


## 0.1.0 development snapshot — 2026-11-12

Source publication on `main`; this snapshot does not introduce a stable binary release or a store submission.

Highlights: independent split panes with session restoration; integrated local Samba share management; network breadcrumbs and SMB browsing; internal Space previews for images, media, PDF, Office/OpenDocument, HTML, Markdown and archives; keyboard preview navigation; consistent toasts and clipboard feedback. Updated installation, update and runtime instructions in Portuguese and English.

- Left-align archive preview names beside their icons and align the Name header with the same column.

- Anchor preview footers independently of loading/content size and center viewer controls. Add Up/Down file navigation in display order and Left/Right page/slide navigation.
- Add read-only archive content previews via an isolated libarchive worker, including ZIP, compressed TAR, 7z and RAR; bound listings, declare runtimes and never extract entries.

- Route Office previews by document family, sharing engines across DOC/DOCX/ODT/RTF, XLS/XLSX/ODS, PPT/PPTX/ODP, Draw/Math documents, templates and Flat XML variants.
- Increase Office preview input capacity to 256 MiB, stream private snapshots with a bounded buffer and allow two minutes for conversion while retaining cancellation. Add legacy/OpenDocument fixtures and runtime dependencies for Draw/Math.

- Add XLSX print-layout previews via LibreOffice Calc and static PPTX previews via Impress, with cached page/slide navigation and declared runtime dependencies.
- Add Markdown Source/Presentation views with formatted headings, lists, tables and code, using the bundled CommonMark parser and the existing presentation engine.
- Fix HTML presentation failures caused by applying screen layouts to paginated output: honor print styles, retain source fallback and replace Python tracebacks with concise localized errors.

- Replace unavailable-preview popups with warning toasts, retaining plain-text detection for unknown extensions and graceful behavior when converters are absent.
- Add formatted DOCX previews via an isolated LibreOffice Writer conversion with cached PDF pages. Combine PDF/document page navigation and zoom into one toolbar.
- Add HTML Source/Presentation switching with static WeasyPrint rendering, cached pages, local-only resources and source fallback; declare document/HTML runtimes in native packages and CI.

- Restore split sessions with each pane's folder, the active side and divider ratio, preserving pane identity after tab reordering and compatibility with older sessions.
- Fit and center image/video previews; size video popups to the decoded aspect ratio with responsive controls instead of a fixed 16:9 canvas. A second Space press closes the preview; key repeat does not toggle it.

- Internal Space/context-menu previews for images, SVG, paginated PDF, bounded text/code and audio/video, with zoom, playback controls, active-pane selection, cancellation and standard error toasts. Declare GStreamer/Poppler runtime dependencies in native packages and exercise real decoders in CI.

- Dim pending cut items in every file view and allow Escape to cancel the pending cut across tabs/panes without altering files or cancelling an already started transfer.

- Fix right-click Paste menus in blank folder space, including switching split panes after Ctrl+C. Exclude file rows/tiles, headers, scrollbars and overlaid windows from blank-space hit testing.

- Give each split pane its own navigation sidebar and paint an opaque theme background, fixing black folder areas in light mode.

- Toolbar toggle for two resizable folder panes in one window, initially showing the same directory. Independent navigation, filtering and selection; per-pane sidebars, shared active-pane toolbar, clipboard and cross-pane drag/drop. Turning the split off preserves both folders as tabs.

- Show network addresses in breadcrumbs, address editing and tab tooltips, and share/folder names in tabs and recently closed tabs. Keep breadcrumb and Up navigation within the remote hierarchy while retaining local paths for filesystem operations.

- Keep the window open when removing a remote sidebar shortcut: defer removal until the sidebar snapshot finishes rendering. Left-align names in the full-width server share list.

- Apply the GVfs fallback to SMB server-root listings as well as share connections, preserving the supplied username in share URLs and respecting cancellation.

- Verify remote folder readability before saving connections; retry failed or unreadable KDE SMB mounts through GVfs when available, supporting temporary authentication without requiring stored passwords.

- Operational errors use the standard error-toast stack; removed the legacy global error banner. SMB share selection now fills the dialog width.

- SMB server browsing from UNC/SMB root addresses, share selection before connecting, and network URL handling in the address bar, with native KDE/GNOME authentication.

- Separate Local shares sidebar section with local folder navigation, independent visibility, immediate manager updates across tabs and periodic background refresh for other windows/external changes.

- Integrated Samba user-share manager: list, create, rename, edit per-user/group permissions, copy SMB/Windows addresses and stop sharing with confirmation, ownership and stale-edit checks, and rename rollback.

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

The first alpha is published for testing; no stable release or store submission is claimed.
