# Interaction

## Escape

One Escape does one thing, in this order:

1. Close the search list. The field keeps its text.
2. Close completion.
3. Close the settings card, the environment card, a structure sheet, a row sheet, the file menu, or the patch dialog.
4. If a run is in progress and Stop is available, cancel it.
5. Otherwise do nothing.

Escape does not close a sheet and cancel the run in the same keypress. The iced window also leaves the inline name and move prompts open on the first Escape; those prompts should cancel, matching rename-in-place.

## Pointer

| Gesture | Result |
| --- | --- |
| Click a file | Open it. |
| Click a directory | Collapse or expand. |
| Double-click a name | Rename in place. |
| Right-click a node | Menu at the click point. The anchor stays if the pointer moves. |
| Right-click the side background | New file and New folder only. |
| Drag a file to a folder or to the tree root | Move. |
| Drag a split | Resize, and write the preference. |
| Double-click a split handle | Reset that preference. |
| Click outside a menu, the search list, or the settings card | Dismiss it. |
| Click the scrim of a sheet | Close the sheet. |
| Click a sheet | The click stays on the sheet. It does not reach the editor, the play button, or a menu underneath. |
| Hover Run | After about one second, tooltip **Run selection**. |
| Hover a row | The eye fades in. |
| Hover a lint mark | The diagnostic. |

Tooltips must not eat the click. A tooltip that appears over the eye, the schema icon, or Run has to forward the press to that control. The iced tooltip delayed 400ms on some controls and 1s on Run; if the tip’s overlay sits on top of the button, the button still activates.

## Overlay order

From back to front:

1. The window.
2. The gutter play button, only while the editor frame is visible and no sheet, menu, search list, or settings card is open.
3. The file menu and the search list.
4. Completion.
5. Settings and the environment card.
6. Structure and row sheets, and the patch dialog.
7. Toasts.

The play button is the case that went wrong in the iced window. It is a float, and floats paint after later siblings, so it sat on top of the structure sheet. Hiding it while a covering surface is open is required even if the sheet is a higher overlay, because a lower float can still receive the press. When the sheet closes, the button comes back in its place beside the frame.

Search and settings also cover the editor. The play button stays hidden for those too.

## Focus

Opening a sheet moves focus to the close control. Tab and Shift+Tab cycle inside the sheet and do not land on the editor behind it.

Editor chords (Ctrl+Enter, Ctrl+\\, Ctrl+S) apply only while the editor is focused. They are bindings on the editor, not a global grab that also fires while the user is typing in search or a rename field.

Arrow keys in the search list and the completion list are handled by those lists, including when the text field would otherwise consume them.

## Running

One run at a time. Starting a run hides Run, shows Stop, disables Run file, and disables the gutter play. The tree dot tracks the file. Progress updates as statements complete. Finish or cancel restores the buttons on the next frame, with no leftover spinner and no leftover progress bar.

## Text entry

Search, the catalog filter, rename, move, and the new-file prompt are text fields. They accept paste. Escape follows the order above. Enter confirms a prompt and opens a search hit. A prompt with an empty name shows **A name is required.**

## Lists that rebuild

The table list, the file tree, and the result cards rebuild when data arrives. Focus, selection, and scroll survive that rebuild. A row’s widget state stays with that row’s identity (the path, the table name, the statement index). Growing the table list must not attach an old row’s state to a new name. The iced window needed an explicit slot around each table row for this. The behavior is the requirement, not the slot.
