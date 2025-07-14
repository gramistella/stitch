import os
import re
import tkinter as tk
from tkinter import filedialog, ttk, scrolledtext, messagebox
from datetime import datetime
import tkinter.font as tkfont

def build_tree_from_paths(paths):
    """
    Given a list of relative file paths like:
      ["README.md", "src/main.py", "src/utils.py", ...]
    Return a nested dictionary representing the tree structure, for example:
      {
        "README.md": {},
        "src": {
          "main.py": {},
          "utils.py": {}
        },
        ...
      }
    """
    tree = {}
    for p in paths:
        # Split on your OS's path separator
        parts = p.split(os.sep)
        current = tree
        for part in parts:
            if part not in current:
                current[part] = {}
            current = current[part]
    return tree

def write_unicode_tree(tree_dict, text_widget, prefix="", is_last=True, root_name=None):
    """
    Recursively write a nested dictionary (tree_dict) using Unicode box-drawing characters
    into the given text_widget (e.g. ScrolledText).

    - prefix: string used for indentation lines (e.g. "│   " or "    ").
    - is_last: True if this node is the last child in its parent, affects "└── " vs "├── ".
    - root_name: if provided, prints that name at the top (useful for a root directory label).
    """
    # If we have a root name (e.g. the top-level folder), write it first
    if root_name is not None:
        text_widget.insert("end", root_name + "\n")

    items = list(tree_dict.items())
    for i, (name, subtree) in enumerate(items):
        last_child = (i == len(items) - 1)
        # Choose └── for the last child, ├── otherwise
        branch_char = "└── " if last_child else "├── "

        # Write this line
        text_widget.insert("end", prefix + branch_char + name + "\n")

        # If we have children, recurse
        if subtree:
            # For children, use "    " if we're the last child, or "│   " otherwise
            child_prefix = prefix + ("    " if last_child else "│   ")
            write_unicode_tree(subtree, text_widget, prefix=child_prefix, is_last=False)


class StitchMainWindow(tk.Tk):
    def __init__(self):
        super().__init__()
        self.tk.call("tk", "scaling", 1.0)
        self.title("Stitch")

        try:
            script_dir = os.path.dirname(__file__)
            icon_path = os.path.join(script_dir, 'assets', 'icon.png')
            self.iconphoto(True, tk.PhotoImage(file=icon_path))
        except tk.TclError:
            print("Warning: Could not find 'assets/icon.png'. Make sure the icon file exists.")

        self.geometry("1100x600")

        #  Bind to the focus event to refresh the window.
        # self.bind("<FocusIn>", self.on_focus_in)

        # Maps tree item IDs (for both file and directory nodes) to their full paths.
        self.tree_filepaths = {}
        # Maps full paths (for both files and directories) to an explicit check state (True/False).
        self.tree_checkstates = {}
        # Maps tree item IDs to their “base” name (without the checkbox prefix).
        self.node_names = {}
        # Tracks the expansion state of directory nodes by their full path.
        self.expanded_paths = {}

        self.selected_directory = None

        # Variable to hold the state of the "Hierarchy Only" checkbox.
        self.hierarchy_only_var = tk.BooleanVar(value=False)
        # Variable to hold the state of the "Directories Only" checkbox.
        self.directories_only_var = tk.BooleanVar(value=False)

        # To keep track of file modification times for auto-update.
        self.last_mod_times = {}
        # Polling interval in milliseconds.
        self.poll_interval = 15000
        # ID for the scheduled polling callback.
        self._poll_id = None

        self._create_widgets()

    def _create_widgets(self):
        # Create a PanedWindow to split the UI into left and right sections.
        self.paned_window = ttk.Panedwindow(self, orient=tk.HORIZONTAL)
        self.paned_window.pack(fill=tk.BOTH, expand=True)

        # Left frame: holds the folder selection button, filter, and the Treeview.
        self.left_frame = ttk.Frame(self.paned_window, width=300)
        self.left_frame.pack(fill=tk.BOTH, expand=True, side=tk.LEFT)

        # Right frame: holds the "Generate Output" button, a label for last refresh, and a scrolled text area.
        self.right_frame = ttk.Frame(self.paned_window)
        self.right_frame.pack(fill=tk.BOTH, expand=True, side=tk.RIGHT)

        self.paned_window.add(self.left_frame, weight=1)
        self.paned_window.add(self.right_frame, weight=3)

        # Button to select the root folder.
        self.select_folder_button = ttk.Button(
            self.left_frame, text="Select Folder", command=self.select_folder
        )
        self.select_folder_button.pack(padx=5, pady=5, anchor="w")

        # Label + Entry for extension filter
        self.filter_label = ttk.Label(self.left_frame, text="Filter Extensions (e.g., .py,.txt):")
        self.filter_label.pack(padx=5, pady=(5, 0), anchor="w")
        self.filter_entry = ttk.Entry(self.left_frame)
        self.filter_entry.pack(fill=tk.X, padx=5, pady=(0, 5))
        self.filter_entry.bind("<KeyRelease>", self.on_filter_change)

        # Label + Entry for excluding directories
        self.exclude_dirs_label = ttk.Label(self.left_frame, text="Exclude Directories (e.g., target,node_modules):")
        self.exclude_dirs_label.pack(padx=5, pady=(5, 0), anchor="w")
        self.exclude_dirs_entry = ttk.Entry(self.left_frame)
        self.exclude_dirs_entry.insert(0, ".git, node_modules, target, _target, .elan, .lake, .idea, .vscode, _app, .svelte-kit, .sqlx, venv")
        self.exclude_dirs_entry.pack(fill=tk.X, padx=5, pady=(0, 5))
        self.exclude_dirs_entry.bind("<KeyRelease>", self.on_filter_change)

        # Label + Entry for excluding files
        self.exclude_files_label = ttk.Label(self.left_frame, text="Exclude Files (e.g., LICENSE):")
        self.exclude_files_label.pack(padx=5, pady=(5, 0), anchor="w")
        self.exclude_files_entry = ttk.Entry(self.left_frame)
        self.exclude_files_entry.insert(0, "LICENSE, Cargo.lock, package-lock.json, yarn.lock, .DS_Store, .dockerignore, .gitignore, .npmignore, .pre-commit-config.yaml, .prettierignore, .prettierrc, eslint.config.js, .env, Thumbs.db")
        self.exclude_files_entry.pack(fill=tk.X, padx=5, pady=(0, 5))
        self.exclude_files_entry.bind("<KeyRelease>", self.on_filter_change)

        # Treeview for hierarchical display of folders and files.
        self.tree = ttk.Treeview(self.left_frame)
        self.tree.pack(fill=tk.BOTH, expand=True, padx=5, pady=5)
        self.tree.bind("<Button-1>", self.on_tree_click)
        self.tree.bind("<<TreeviewOpen>>", self.on_open)
        self.tree.bind("<<TreeviewClose>>", self.on_close)

        self.tree_font = tkfont.nametofont("TkDefaultFont")
        self.checkbox_width = self.tree_font.measure("[ ] ")

        style = ttk.Style()
        style.configure("Treeview", font=self.tree_font)

        # Controls for the right-hand side
        right_controls_frame = ttk.Frame(self.right_frame)
        right_controls_frame.pack(fill=tk.X, padx=5, pady=5)

        self.generate_button = ttk.Button(
            right_controls_frame, text="Generate Output", command=self.generate_output
        )
        self.generate_button.pack(side=tk.LEFT, anchor="w")

        self.select_from_text_button = ttk.Button(
            right_controls_frame, text="Select from Text...", command=self.open_select_from_text_dialog
        )
        self.select_from_text_button.pack(side=tk.LEFT, padx=10, anchor="w")

        # Checkbox for hierarchy-only output
        self.hierarchy_only_checkbox = ttk.Checkbutton(
            right_controls_frame, text="Hierarchy Only", variable=self.hierarchy_only_var
        )
        self.hierarchy_only_checkbox.pack(side=tk.LEFT, padx=10, anchor="w")

        # Checkbox for directories-only output
        self.directories_only_checkbox = ttk.Checkbutton(
            right_controls_frame, text="Directories Only", variable=self.directories_only_var
        )
        self.directories_only_checkbox.pack(side=tk.LEFT, padx=10, anchor="w")

        self.last_refresh_label = ttk.Label(self.right_frame, text="Last refresh: N/A")
        self.last_refresh_label.pack(padx=5, pady=(0, 5), anchor="w")

        self.output_text = scrolledtext.ScrolledText(self.right_frame, wrap=tk.WORD)
        self.output_text.pack(fill=tk.BOTH, expand=True, padx=5, pady=5)


    # Helper method: Parses the file exclusion list from the UI.
    def get_excluded_files(self):
        """Parses the comma-separated list of filenames from the entry widget."""
        exclude_str = self.exclude_files_entry.get().strip()
        if not exclude_str:
            return set()
        return {name.strip() for name in exclude_str.split(",")}

    # Helper method: Parses the directory exclusion list from the UI.
    def get_excluded_dirs(self):
        """Parses the comma-separated list of directory names from the entry widget."""
        exclude_str = self.exclude_dirs_entry.get().strip()
        if not exclude_str:
            return set()
        return {name.strip() for name in exclude_str.split(",")}

    def on_filter_change(self, event=None):
        """Called when the filter text changes. If a folder is selected, refresh the tree."""
        if self.selected_directory:
            self.refresh_tree()

    def update_last_refresh_time(self):
        now_str = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
        self.last_refresh_label.config(text=f"Last refresh: {now_str}")

    def select_folder(self):
        """Prompt the user to select a folder and populate the tree view with its hierarchical structure."""
        folder_selected = filedialog.askdirectory()
        if folder_selected:
            self.selected_directory = folder_selected
            # Clear existing tree items and state (except check states, which we preserve)
            self.tree.delete(*self.tree.get_children())
            self.tree_filepaths.clear()
            self.node_names.clear()
            # Do not clear self.tree_checkstates so that explicit states are preserved across refreshes.
            self.last_mod_times.clear()
            # Cancel any pending polling callbacks.
            if self._poll_id is not None:
                try:
                    self.after_cancel(self._poll_id)
                except Exception:
                    pass
                self._poll_id = None
            # Populate the tree with the folder structure.
            self.refresh_tree()
            self.update_last_refresh_time()
            # Start polling for updates.
            self._poll_id = self.after(self.poll_interval, self.check_updates)

    def _populate_tree(self, path, parent, filters, exclude_dirs, exclude_files):
        """
        Recursively populates the tree, applying exclusions and optional extension filters.
        Returns True if any item was added, False otherwise.
        """
        if os.path.basename(path) in exclude_dirs and path != self.selected_directory:
            return False

        try:
            items = sorted(os.listdir(path))
        except PermissionError:
            return False

        was_item_added = False
        for item in items:
            full_path = os.path.join(path, item)
            is_dir = os.path.isdir(full_path)

            # Apply exclusion and filter logic
            if is_dir:
                if item in exclude_dirs:
                    continue
                # If using filters, check if the directory contains any matching children
                if filters and not self.directory_contains_match(full_path, filters):
                    continue
            else: # Is a file
                if item in exclude_files:
                    continue
                # If using filters, check if the file extension matches
                if filters:
                    ext = os.path.splitext(item)[1].lower()
                    if ext not in filters:
                        continue

            # Determine checkbox state
            if full_path in self.tree_checkstates:
                effective_state = self.tree_checkstates[full_path]
            elif parent:
                effective_state = self.get_effective_state(parent)
            else:
                effective_state = False

            # Insert the node into the tree
            prefix = "[x] " if effective_state else "[ ] "
            display_text = prefix + item
            open_state = self.expanded_paths.get(full_path, True) if is_dir else False

            node_id = self.tree.insert(parent, "end", text=display_text, open=open_state)
            self.tree_filepaths[node_id] = full_path
            self.node_names[node_id] = item
            was_item_added = True

            if is_dir:
                self._populate_tree(full_path, node_id, filters, exclude_dirs, exclude_files)

        return was_item_added

    def directory_contains_match(self, path, filters):
        exclude_dirs = self.get_excluded_dirs()
        exclude_files = self.get_excluded_files()

        try:
            items = os.listdir(path)
        except PermissionError:
            return False

        for item in items:
            full_path = os.path.join(path, item)
            is_dir = os.path.isdir(full_path)
            if is_dir:
                if item in exclude_dirs:
                    continue
                if self.directory_contains_match(full_path, filters):
                    return True
            else:
                if item in exclude_files:
                    continue
                if not filters:
                    return True
                ext = os.path.splitext(item)[1].lower()
                if ext in filters:
                    return True
        return False

    def refresh_tree(self):
        """Refresh the tree view, creating a node for the root directory first."""
        for i in self.tree.get_children():
            self.tree.delete(i)

        self.tree_filepaths.clear()
        self.node_names.clear()

        if self.selected_directory:
            root_name = os.path.basename(self.selected_directory)
            effective_state = self.tree_checkstates.get(self.selected_directory, False)
            prefix = "[x] " if effective_state else "[ ] "
            display_text = prefix + root_name

            root_id = self.tree.insert("", "end", text=display_text, open=True)
            self.tree_filepaths[root_id] = self.selected_directory
            self.node_names[root_id] = root_name

            # Get filters and exclusions once
            filter_str = self.filter_entry.get().strip()
            filters = [f.strip().lower() for f in filter_str.split(",") if f.strip()] if filter_str else None
            exclude_dirs = self.get_excluded_dirs()
            exclude_files = self.get_excluded_files()

            # Always call the single, unified population method
            self._populate_tree(
                self.selected_directory,
                parent=root_id,
                filters=filters,
                exclude_dirs=exclude_dirs,
                exclude_files=exclude_files
            )

        self.update_last_refresh_time()

    def get_effective_state(self, item_id):
        """
        Return the effective check state for the given node by checking whether an explicit state
        has been set for this node or any ancestor. Default to False if none is found.
        """
        full_path = self.tree_filepaths.get(item_id)
        if full_path in self.tree_checkstates:
            return self.tree_checkstates[full_path]
        parent_id = self.tree.parent(item_id)
        if parent_id:
            return self.get_effective_state(parent_id)
        return False

    def update_tree_item_display(self, item_id):
        """Update the display text for a given tree item based on its effective check state."""
        effective_state = self.get_effective_state(item_id)
        prefix = "[x] " if effective_state else "[ ] "
        base_name = self.node_names.get(item_id, "")
        self.tree.item(item_id, text=prefix + base_name)

    def update_tree_item_recursive(self, item_id):
        """Recursively update the display text for this node and all its descendants."""
        self.update_tree_item_display(item_id)
        for child in self.tree.get_children(item_id):
            self.update_tree_item_recursive(child)

    def on_tree_click(self, event):
        # Get the current pointer coordinates relative to the tree widget.
        x = self.tree.winfo_pointerx() - self.tree.winfo_rootx()
        y = self.tree.winfo_pointery() - self.tree.winfo_rooty()

        region = self.tree.identify("region", x, y)
        if region != "tree":
            return

        item_id = self.tree.identify_row(y)
        if not item_id or item_id not in self.tree_filepaths:
            return

        # Get the bounding box for the item in column "#0"
        bbox = self.tree.bbox(item_id, "#0")
        if not bbox:
            return
        bx, by, width, height = bbox

        # Use the bounding box's x coordinate and an adjustment to locate the checkbox region.
        adjustment = 10  # needed to align things correctly
        checkbox_left = bx + adjustment
        checkbox_right = bx + adjustment + self.checkbox_width

        # If the current pointer x falls within the checkbox region, toggle the state.
        if checkbox_left <= x <= checkbox_right:
            effective_state = self.get_effective_state(item_id)
            new_state = not effective_state
            full_path = self.tree_filepaths[item_id]
            self.tree_checkstates[full_path] = new_state

            # If this is a directory, clear explicit check states of all descendant nodes,
            # so that they inherit the new state.
            if os.path.isdir(full_path):
                self.clear_descendant_explicit_states(item_id)

            self.update_tree_item_recursive(item_id)
            self.generate_output()
            return "break"

    def clear_descendant_explicit_states(self, item_id):
        """
        Recursively remove the explicit check state for all descendant nodes.
        This ensures that toggling the parent overrides any manual changes in its children.
        """
        for child in self.tree.get_children(item_id):
            child_path = self.tree_filepaths.get(child)
            if child_path in self.tree_checkstates:
                del self.tree_checkstates[child_path]
            # Recurse into deeper levels
            self.clear_descendant_explicit_states(child)

    def generate_output(self):
        self.output_text.delete("1.0", tk.END)
        if not self.selected_directory:
            self.output_text.insert(tk.END, "No folder selected.\n")
            return

        items_for_hierarchy = []
        # If "Directories Only" is checked, collect directories.
        if self.directories_only_var.get():
            for item_id, full_path in self.tree_filepaths.items():
                if os.path.isdir(full_path) and self.get_effective_state(item_id):
                    items_for_hierarchy.append(full_path)
        # Otherwise, collect files.
        else:
            for item_id, full_path in self.tree_filepaths.items():
                if os.path.isfile(full_path) and self.get_effective_state(item_id):
                    items_for_hierarchy.append(full_path)

        if not items_for_hierarchy:
            self.output_text.insert(tk.END, "No items selected.\n")
            self.update_last_refresh_time()
            return

        items_for_hierarchy.sort(key=lambda p: os.path.relpath(p, self.selected_directory))

        # Update last modification times for selected files (if not in directories-only mode)
        if not self.directories_only_var.get():
            selected_files = items_for_hierarchy
            for filepath in selected_files:
                try:
                    self.last_mod_times[filepath] = os.path.getmtime(filepath)
                except Exception:
                    self.last_mod_times[filepath] = None

        self.output_text.insert(tk.END, "=== FILE HIERARCHY ===\n\n")

        relative_paths = []
        root_name = os.path.basename(self.selected_directory)
        for fp in items_for_hierarchy:
            rp = os.path.relpath(fp, self.selected_directory)
            # Avoid adding a "." for the root directory itself in the hierarchy list
            if rp == ".":
                continue
            parts = rp.split(os.sep)
            new_rp = os.sep.join(parts)
            relative_paths.append(new_rp)

        tree_dict = build_tree_from_paths(relative_paths)
        write_unicode_tree(tree_dict, self.output_text, root_name=root_name)

        # Only append file contents if neither of the "only" checkboxes are ticked.
        if not self.hierarchy_only_var.get() and not self.directories_only_var.get():
            self.output_text.insert(tk.END, "\n=== FILE CONTENTS ===\n\n")
            selected_files = items_for_hierarchy
            for filepath in selected_files:
                rel_path = os.path.relpath(filepath, self.selected_directory)
                self.output_text.insert(tk.END, f"--- Start of file: {rel_path} ---\n")
                try:
                    with open(filepath, "r", encoding="utf-8", errors="replace") as f:
                        contents = f.read()
                except Exception as e:
                    contents = f"Error reading file: {e}"
                self.output_text.insert(tk.END, contents + "\n")
                self.output_text.insert(tk.END, f"--- End of file: {rel_path} ---\n\n")

        self.output_text.focus()

        if self._poll_id is not None:
            try:
                self.after_cancel(self._poll_id)
            except Exception:
                pass
            self._poll_id = None
        self._poll_id = self.after(self.poll_interval, self.check_updates)
        self.update_last_refresh_time()

    # Modified check_updates: Prunes excluded directories from the os.walk.
    def check_updates(self):
        if not self.selected_directory:
            return

        exclude_dirs = self.get_excluded_dirs()
        exclude_files = self.get_excluded_files()

        current_paths = set(self.tree_filepaths.values())
        new_paths = set()
        for root, dirs, files in os.walk(self.selected_directory):
            dirs[:] = [d for d in dirs if d not in exclude_dirs]
            for d in dirs:
                new_paths.add(os.path.join(root, d))
            for f in files:
                if f not in exclude_files:
                    new_paths.add(os.path.join(root, f))

        if current_paths != new_paths:
            self.refresh_tree()
            if self._poll_id is not None:
                self.after_cancel(self._poll_id)
            self._poll_id = self.after(self.poll_interval, self.check_updates)
            return

        selected_files = []
        for item_id, filepath in self.tree_filepaths.items():
            if os.path.isfile(filepath) and self.get_effective_state(item_id):
                selected_files.append(filepath)

        update_needed = False
        for filepath in selected_files:
            try:
                current_mod_time = os.path.getmtime(filepath)
            except Exception:
                current_mod_time = None
            last_mod_time = self.last_mod_times.get(filepath)
            if current_mod_time != last_mod_time:
                update_needed = True
                break

        if update_needed:
            self.generate_output()
        else:
            if self._poll_id is not None:
                try:
                    self.after_cancel(self._poll_id)
                except Exception:
                    pass
            self._poll_id = self.after(self.poll_interval, self.check_updates)

    def on_open(self, event):
        """Update the expansion state when a directory is expanded."""
        item_id = self.tree.focus()
        if item_id and item_id in self.tree_filepaths:
            full_path = self.tree_filepaths[item_id]
            if os.path.isdir(full_path):
                self.expanded_paths[full_path] = True

    def on_close(self, event):
        """Update the expansion state when a directory is collapsed."""
        item_id = self.tree.focus()
        if item_id and item_id in self.tree_filepaths:
            full_path = self.tree_filepaths[item_id]
            if os.path.isdir(full_path):
                self.expanded_paths[full_path] = False

    def on_focus_in(self, event=None):
        # Force a geometry recalculation as above.
        current_width = self.winfo_width()
        current_height = self.winfo_height()
        self.geometry(f"{current_width + 1}x{current_height}")
        self.after(50, lambda: self.geometry(f"{current_width}x{current_height}"))
        self.tk.call("tk", "scaling", 1.0)
        self.tree.update_idletasks()
        # Optionally, refresh the tree entirely.
        if self.selected_directory:
            self.refresh_tree()

    def open_select_from_text_dialog(self):
        """Opens a dialog to paste hierarchy text for auto-selection."""
        if not self.selected_directory:
            messagebox.showwarning("Warning", "Please select a root folder first.")
            return

        dialog = tk.Toplevel(self)
        dialog.title("Select from Hierarchy Text")
        dialog.geometry("500x500")

        label = ttk.Label(dialog, text="Paste hierarchy text below (must include root folder name):")
        label.pack(padx=10, pady=(10, 5), anchor="w")

        text_area = scrolledtext.ScrolledText(dialog, wrap=tk.WORD)
        text_area.pack(padx=10, pady=5, expand=True, fill=tk.BOTH)
        text_area.focus()

        apply_button = tk.Button(dialog, text="Apply and Select", command=lambda: self._apply_and_close(text_area.get("1.0", tk.END), dialog))
        apply_button.pack(pady=10, padx=5)

    def _apply_and_close(self, text, dialog):
        """Helper to apply selections and then close the dialog."""
        self.apply_selection_from_text(text)
        dialog.destroy()

    def apply_selection_from_text(self, text):
        """Parses the hierarchy text and updates the tree's check states."""
        # 1. Parse the text to get a set of desired relative paths.
        try:
            # We no longer need to manually add the root ".", as we only care about files.
            parsed_relative_paths = self._parse_hierarchy_text(text)

        except Exception as e:
            messagebox.showerror("Parsing Error", f"Could not parse the hierarchy text.\n\nError: {e}")
            return

        # 2. Clear all previous explicit check states.
        self.tree_checkstates.clear()

        # 3. Refresh the tree completely. It will be drawn with default (unchecked) states.
        self.refresh_tree()

        # 4. Populate checkstates for FILES ONLY.
        paths_to_check = 0
        for item_id, full_path in self.tree_filepaths.items():
            try:
                item_rel_path = os.path.relpath(full_path, self.selected_directory)
                normalized_item_rel_path = item_rel_path.replace(os.sep, "/")

                if normalized_item_rel_path in parsed_relative_paths:
                    # --- THE CORE FIX ---
                    # Only set the state to True if the path is a file. Ignore directories.
                    if os.path.isfile(full_path):
                        self.tree_checkstates[full_path] = True
                        paths_to_check += 1
            except ValueError:
                continue

        # 5. Directly update the display for each item, ignoring inheritance.
        for item_id, full_path in self.tree_filepaths.items():
            is_checked = self.tree_checkstates.get(full_path, False)
            prefix = "[x] " if is_checked else "[ ] "
            base_name = self.node_names.get(item_id)
            if base_name:
                self.tree.item(item_id, text=prefix + base_name)

        # 6. Generate the final output.
        self.generate_output()

    def _parse_hierarchy_text(self, text):
        """Parses a string with a tree structure into a set of relative paths."""
        lines = text.strip().split('\n')
        if not lines or not lines[0].strip():
            raise ValueError("The first line must contain the root directory name.")

        name_finder_regex = re.compile(r"[^│└──├\s]")
        relative_paths = set()
        path_parts = [] # Tracks current path components, e.g., ['backend', 'src']

        for line in lines[1:]: # Skip the root directory name on the first line
            if not line.strip():
                continue

            match = name_finder_regex.search(line)
            if not match:
                continue

            name_start_index = match.start()
            level = (name_start_index - 1) // 4 if name_start_index > 0 else 0
            name = line[name_start_index:].strip()

            path_parts = path_parts[:level]
            path_parts.append(name)

            current_rel_path = os.path.join(*path_parts)
            relative_paths.add(current_rel_path)


        return relative_paths


def main():
    app = StitchMainWindow()
    app.mainloop()

if __name__ == "__main__":
    main()