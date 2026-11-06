# Files

The side column in Query mode is the workspace passed to the process (`--dir`). The tree shows directories and `.dql` files. The fixture the tests assume is `empty/` (it contains `.gitkeep`), `queries/edit.dql`, `queries/read.dql`, and `queries/functions.dql`. On launch the page opens `edit.dql`. The window opens the first file.

A node id is the workspace-relative path. Tests use `data-testid="node-{path}"` with `/` replaced by `__`.

Directories collapse and expand. Clicking a directory toggles its children. Clicking a file opens it, selects the row, and moves the open-file marks (see [02-shell.md](02-shell.md)).

## Creating

**+** in the side head prompts for a file. The prompt default is `queries/notes.dql` when the selection is at the workspace root in the page’s “new file” flow that targets the queries folder; accepting `top.dql` creates that file and opens it.

The root context menu (right-click the side background, or the side head) offers only **New file** and **New folder**. It does not offer Duplicate, Run, Clear results, or Clear cache. A folder prompt defaults to `folder`; accepting `extra` creates `extra`.

Right-click a directory to create inside it. The file prompt then defaults to `notes.dql`, and accepting `made.dql` creates `queries/made.dql` when the directory is `queries`.

Right-click a file to create a sibling: the new file’s directory is the file’s parent.

Names: empty is rejected with **A name is required.** The iced defaults when the prompt has no context are `new.dql` and `folder`.

## Menu

Right-click a node. The menu is a floating card anchored at the pointer where the click happened. Later pointer motion does not move it. Clicking elsewhere dismisses it. On a file, the page shows:

| Item | Notes |
| --- | --- |
| Run file | Danger color, fast-forward icon. Opens that file and runs every statement. |
| New file | |
| New folder | |
| Duplicate | |
| Rename | |
| Move | The page’s menu label in script is **Move to…**. The iced window says **Move**. Use **Move to…**. |
| Clear results | Clears that file’s query-result cache and its cards. |
| Delete | Danger color. |

On a directory: **Run folder** instead of **Run file**, plus **New file**, **New folder**, **Duplicate**, **Rename**, **Move to…**, **Clear results**, **Delete**. No **Clear cache** on either menu. Clear cache lives in settings.

The iced file menu drops **New folder** and **Clear results**, and the directory menu adds **Clear results** which the page also has. Match the page.

Run file from the menu leaves the user on that file. A four-statement file yields four success marks. Run folder walks `.dql` files recursively. Each file’s results stay with that file. An empty folder reports **Nothing to run** and does not show a running dot.

## Rename, duplicate, move, delete

Double-click a name to rename in place. Escape cancels. Enter commits. `empty` can become `vacant`. `queries/edit.dql` can become `queries/renamed.dql`.

Duplicate appends ` copy` before the extension: `queries/edit copy.dql`, and `empty copy` for a folder. The new file opens.

Move asks for a destination path. The prompt default is the current path. `queries/read.dql` can become `empty/moved.dql`.

Drag a file onto a folder to move it (`empty/edit.dql`). Drag onto the empty bottom of the tree to move it to the workspace root. The iced window does not drag. The rewrite does.

Delete removes a file. Delete removes a directory that contains only `.dql` files (or is empty aside from those). Delete refuses a directory that holds a non-`.dql` file. The confirm text contains `folder contains .gitkeep, which is not a .dql file`, and the directory stays. The same rule keeps `readme.txt`.

## Save and watch

Edits autosave. The page waits 400ms and PUTs the file. The iced window saves on edit. Either way the workspace file matches the buffer within a few seconds, and a clean external write updates the buffer with no dialog.

If the buffer is dirty and the file changes on disk, the disk copy is what the buffer shows and the unsaved text is held as a patch. The page dialog title is **File changed on disk**. The body shows a unified diff (`+-- local unsaved` in the test). Actions: recall the patch, step between patches, copy the patch. Recalling does not write the patch back to disk until the user saves again. The button reads **Unsaved patch**, or **N unsaved patches**.

The iced window uses a thinner strip: **This file changed on disk. Your edits were kept.** with **Load disk** and **Keep mine**. The rewrite uses the page dialog. Two windows on one file follow the same rule: the clean window picks up the save, the dirty window gets the patch, and the disk keeps the saved text.

## Running mark on the tree

While a file runs, that row shows one blue dot (`--blue`), pulsing. No other row shows it. It clears when the run finishes. Idle shows zero running dots.
